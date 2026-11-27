//! The camera component: which part of the world a running game sees.
//!
//! The editor's own camera is not this. That one belongs to the tool and lives
//! in `voltra-editor::camera::ViewportCamera`; this one is authored, saved with
//! the scene and is what a shipped game renders through. Unity, Godot, Unreal
//! and Bevy all draw the same line — a scene camera is a component (or node,
//! or actor) in the world, and the editor's viewport camera is not in the world
//! at all.

use voltra_ecs::{Entity, World};
use voltra_render::View2D;

mod placement;
mod shots;
pub mod target;
mod viewport;

pub use placement::{aspect, eye, normalised_turn, placement, UPRIGHT};
pub use shots::{shots, Shot};
pub use target::CameraTarget;
pub use viewport::Viewport;

/// A camera authored into the scene.
///
/// Its position comes from the entity's [`Transform`], composed through any
/// parents, so a camera can be hung off the thing it follows without a follow
/// system existing yet. An entity with a `Camera` and no `Transform` sits at
/// the world origin, which is what the identity matrix already means
/// everywhere else.
///
/// [`Transform`]: crate::transform::Transform
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Camera {
    /// Half the world-space height the camera shows.
    ///
    /// Unity's `orthographicSize`, and stored the way Unity stores it rather
    /// than as [`View2D::zoom`]'s reciprocal: "this camera sees five units
    /// of world" is a sentence about the scene, while a zoom factor is a
    /// sentence about a viewport. `view` converts.
    pub size: f32,
    /// Which camera wins when more than one is active. Higher takes the frame.
    ///
    /// Unity's `depth` and Bevy's `order`. An `i32` rather than an `f32` so
    /// ties are exact, the same reason [`Sprite::sort_order`] is one.
    ///
    /// [`Sprite::sort_order`]: crate::sprite::Sprite::sort_order
    pub priority: i32,
    /// Whether this camera is a candidate at all.
    ///
    /// Godot's `enabled`, Bevy's `is_active`. A scene usually holds several
    /// framings and renders through one; deleting the others to choose is not
    /// a choice anyone should have to make.
    pub active: bool,
    /// How the frame seen through this camera is graded. Neutral, and not
    /// written, unless set.
    #[serde(default, skip_serializing_if = "crate::post::PostProcess::is_neutral")]
    pub post: crate::post::PostProcess,
    /// The light everything seen through this camera gets, linear; with it
    /// the frame is lit and every [`PointLight2D`](crate::PointLight2D)
    /// adds to it. `None` — the default, and not written — is unlit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient: Option<[f32; 3]>,
    /// Shadow map texels across each quarter turn around a light that casts
    /// shadows, when the frame is lit: sharper edges for more memory and
    /// fill. The default is not written; the renderer clamps it to the
    /// device.
    #[serde(
        default = "default_shadow_resolution",
        skip_serializing_if = "is_default_shadow_resolution"
    )]
    pub shadow_resolution: u32,
    /// The part of the screen it draws into: the whole of it by default —
    /// not written then — or a part, for split screen and a picture in a
    /// picture.
    #[serde(default, skip_serializing_if = "Viewport::is_full")]
    pub viewport: Viewport,
    /// Snaps the view to the target's pixel grid, so pixel art stays crisp
    /// and does not shimmer as the camera moves. Pair it with nearest
    /// texture filtering. Off while the view is turned — a turned view's
    /// pixels do not run along the world's grid — and back once it is
    /// upright. Not written when off.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pixel_perfect: bool,
    /// Whether the view turns with the entity's parents as well as with its
    /// own rotation — Godot's `ignore_rotation` the other way up. Off by
    /// default, and not written then: a camera hung off a rolling ball or a
    /// character turning with its floor stays upright, as every camera did
    /// before views turned; its own rotation always turns it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub inherit_rotation: bool,
}

fn default_shadow_resolution() -> u32 {
    voltra_render::lights::DEFAULT_SHADOW_RESOLUTION
}

fn is_default_shadow_resolution(texels: &u32) -> bool {
    *texels == default_shadow_resolution()
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            // One world unit above and below the centre, which is exactly what
            // `View2D::default` shows. A camera dropped into a scene frames
            // what the editor was already framing at its home zoom.
            size: 1.0,
            priority: 0,
            active: true,
            post: crate::post::PostProcess::default(),
            ambient: None,
            shadow_resolution: default_shadow_resolution(),
            viewport: Viewport::FULL,
            pixel_perfect: false,
            inherit_rotation: false,
        }
    }
}

impl Camera {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "Camera";

    /// Bounds on [`Self::size`], mirroring [`View2D`]'s on zoom.
    ///
    /// The reciprocal of the pair over there, because that is the same
    /// constraint written in this type's units: `View2D::set_zoom` clamps
    /// whatever `view` hands it, so these exist to say what the field means,
    /// and so a UI can stop a drag at the wall rather than at infinity.
    pub const MIN_SIZE: f32 = 1.0 / View2D::MAX_ZOOM;
    /// The largest [`Self::size`] the camera's zoom limit allows.
    pub const MAX_SIZE: f32 = 1.0 / View2D::MIN_ZOOM;

    /// A camera with `size` and every other field at its default.
    pub fn new(size: f32) -> Self {
        Self {
            size,
            ..Default::default()
        }
    }

    /// Sets the draw order among cameras; see [`Camera::priority`].
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// The zoom [`View2D`] wants for this size.
    ///
    /// A zero or negative size produces an infinity, and a `NaN` size produces
    /// a `NaN`. Both are handed straight to `View2D::set_zoom`, which clamps
    /// the first and refuses the second — guarding again here would mean two
    /// places deciding what a broken camera does.
    pub fn zoom(&self) -> f32 {
        1.0 / self.size
    }
}

/// The camera a game renders through: the highest-priority active one.
///
/// `None` when the scene holds no active camera, which is a state to report
/// rather than to paper over — Unity's Game view says "No cameras rendering"
/// instead of inventing a framing, because a scene that draws nothing has a
/// cause the author can fix.
///
/// Ties break on [`Entity::index`], ascending, so the answer does not depend on
/// query order. Priority is the control for when the tie matters.
pub fn active(world: &World) -> Option<Entity> {
    world
        .query::<Camera>()
        .filter(|(_, camera)| camera.active)
        .min_by_key(|(entity, camera)| (-camera.priority, entity.index()))
        .map(|(entity, _)| entity)
}

/// The view `entity`'s camera describes, at `aspect`.
///
/// `None` if it carries no [`Camera`]. The view sits where the entity is,
/// turned by its own rotation — and its parents', with
/// [`Camera::inherit_rotation`] — with its [`CameraShake`] on top ([`eye`]).
/// Scale is read for nothing: how much a camera shows is its
/// [`Camera::size`], and a mirrored camera is turned, not flipped.
///
/// [`CameraShake`]: crate::CameraShake
pub fn view(world: &World, entity: Entity, aspect: f32) -> Option<View2D> {
    let camera = *world.get::<Camera>(entity)?;
    let (position, rotation) = eye(world, entity);
    Some(View2D::new(position, camera.zoom(), aspect).with_rotation(rotation))
}

/// The view the scene renders through at `aspect`: [`active`], then [`view`].
///
/// `None` when no camera is active, which is the state both callers have to
/// answer for — the editor's game view keeps the last framing and says so in
/// words, and the player keeps the default framing and logs it once. They
/// answer it differently, so this composes the two lookups and nothing more.
pub fn active_view(world: &World, aspect: f32) -> Option<View2D> {
    view(world, active(world)?, aspect)
}

#[cfg(test)]
mod placement_tests;
#[cfg(test)]
mod tests;
