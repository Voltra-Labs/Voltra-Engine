//! The sprite component: a coloured quad, sized by its entity's transform.

pub mod quad;
pub mod sheets;

use voltra_assets::{AssetPath, Atlas, Atlases, Frame, Handle, Textures};
use voltra_ecs::Entity;
use voltra_render::Texture;

/// A coloured quad. Its size comes from the entity's [`Transform`].
///
/// [`Transform`]: crate::transform::Transform
///
/// No longer `Copy`: `texture_handle` is an `Option<Handle<Texture>>`, and
/// while a `Handle` itself is `Copy`, wrapping every field of a type in a
/// blanket "and this stays `Copy`" invites forgetting that a handle addresses
/// a GPU resource, not a value to duplicate freely. `Clone` is enough for
/// every caller that used to rely on `Copy`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Sprite {
    /// Multiplied with the bound texture. White leaves the texture as-is.
    pub color: [f32; 4],
    /// Draw order within the scene. Higher is drawn later, and therefore on
    /// top.
    ///
    /// A sorting key, **not** a coordinate — this is a 2D engine with no depth
    /// buffer, and nothing here corresponds to an axis. Named after Unity's
    /// `sortingOrder` rather than Godot's `z_index` for exactly that reason:
    /// when a real Z eventually exists, the name must still be free.
    ///
    /// An `i32` rather than an `f32` so ties are exact and never depend on a
    /// float's representation.
    ///
    /// Sprites sharing a `sort_order` draw in [`Entity`] index order, and an
    /// index is fixed for its entity's whole lifetime — but indices are
    /// recycled LIFO, so a sprite spawned after a despawn can inherit a low
    /// index and draw behind older sprites it was created after. `sort_order`
    /// is the control for when that matters; index order is only the tiebreak
    /// for when it does not.
    pub sort_order: i32,
    /// The texture this sprite names, by path relative to the asset root.
    ///
    /// `None` draws with `color` alone, against the renderer's white bind
    /// group. Serialised: this is the only form a scene file can carry, since
    /// a path survives a reload and a GPU handle does not.
    #[serde(default)]
    pub texture: Option<AssetPath>,
    /// The GPU handle `texture` currently resolves to, if any.
    ///
    /// Never serialised — a handle addresses a slot in whichever `Textures`
    /// loaded this session, which means nothing across a save/load or even
    /// across two runs of the same process. Resolved fresh from `texture` on
    /// load, by [`set_texture`](Sprite::set_texture) or its world-wide caller.
    #[serde(skip)]
    pub texture_handle: Option<Handle<Texture>>,
    /// The slicing this sprite takes its frame from, by path.
    ///
    /// `None` draws the whole texture, which is what every sprite written
    /// before atlases existed does.
    #[serde(default)]
    pub atlas: Option<AssetPath>,
    /// The handle `atlas` currently resolves to, if any.
    ///
    /// Never serialised, for the same reason `texture_handle` is not: it
    /// addresses a slot in whichever store loaded this session.
    #[serde(skip)]
    pub atlas_handle: Option<Handle<Atlas>>,
    /// Which frame of the atlas to draw. Ignored without an atlas.
    ///
    /// An index past the last frame draws the placeholder rather than the
    /// nearest frame: clamping would hide an authoring mistake in the one
    /// place it would have been seen.
    #[serde(default)]
    pub frame: u32,
    /// A rectangle of the texture, in texels, for a sheet with no atlas file.
    ///
    /// Godot's `region_rect` and Bevy's `Sprite::rect`, and worth keeping
    /// beside the atlas: a UI element cut once out of a shared sheet does not
    /// need a file of its own. The atlas wins when both are set — a sprite
    /// that names one is asking for its frames.
    #[serde(default)]
    pub region: Option<Frame>,
    /// Mirrors the drawn image horizontally, without touching the transform.
    ///
    /// Explicit rather than a negative scale, which is how Unity, Godot and
    /// Bevy all spell it: scale is shared with the collider and the gizmo, and
    /// turning a character round must not resize its physics.
    #[serde(default)]
    pub flip_x: bool,
    /// Mirrors the drawn image vertically. See [`flip_x`](Self::flip_x).
    #[serde(default)]
    pub flip_y: bool,
    /// Texels per world unit.
    ///
    /// `None` keeps the original rule — the quad is one unit square and the
    /// transform's scale is its size — so every scene already on disk draws
    /// exactly as it did. `Some(ppu)` sizes the quad from the frame instead,
    /// which is what keeps the frames of a sheet with different-sized cells
    /// from stretching to a common square.
    #[serde(default)]
    pub pixels_per_unit: Option<f32>,
}

impl Default for Sprite {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0, 1.0],
            sort_order: 0,
            texture: None,
            texture_handle: None,
            atlas: None,
            atlas_handle: None,
            frame: 0,
            region: None,
            flip_x: false,
            flip_y: false,
            pixels_per_unit: None,
        }
    }
}

impl Sprite {
    /// Half the width and height of the quad a sprite covers, before its
    /// transform is applied.
    ///
    /// Batching and picking must agree on this. If they ever disagree, a click
    /// lands somewhere other than the pixels it appears to land on, and nothing
    /// reports it.
    pub const HALF_EXTENT: f32 = 0.5;

    pub fn new(color: [f32; 4]) -> Self {
        Self {
            color,
            ..Default::default()
        }
    }

    pub fn with_sort_order(mut self, order: i32) -> Self {
        self.sort_order = order;
        self
    }

    /// Sets or clears the texture path and refreshes the runtime handle.
    ///
    /// `None` clears both fields so the renderer uses its white bind group.
    /// `Some(path)` stores the path and loads through `textures` (which may
    /// return the placeholder handle on failure).
    pub fn set_texture(
        &mut self,
        path: Option<AssetPath>,
        textures: &mut Textures,
        device: &voltra_render::wgpu::Device,
        queue: &voltra_render::wgpu::Queue,
    ) {
        match path {
            None => {
                self.texture = None;
                self.texture_handle = None;
            }
            Some(path) => {
                let handle = textures.load(device, queue, &path);
                self.texture = Some(path);
                self.texture_handle = Some(handle);
            }
        }
    }

    /// Sets or clears the atlas path and refreshes the runtime handle.
    ///
    /// No device, unlike [`set_texture`](Self::set_texture): a slicing is
    /// arithmetic, and `Atlases` needs no GPU to read one.
    pub fn set_atlas(&mut self, path: Option<AssetPath>, atlases: &mut Atlases) {
        match path {
            None => {
                self.atlas = None;
                self.atlas_handle = None;
            }
            Some(path) => {
                let handle = atlases.load(&path);
                self.atlas = Some(path);
                self.atlas_handle = Some(handle);
            }
        }
    }
}

/// The ordering both drawing and picking use: `sort_order` first, then the
/// entity's index to break ties.
///
/// One function rather than two matching tuples. If batching and picking ever
/// disagreed, a click would select something other than the sprite whose pixels
/// are visible, and nothing would report it.
pub fn draw_key(entity: Entity, sprite: &Sprite) -> (i32, u32) {
    (sprite.sort_order, entity.index())
}

#[cfg(test)]
mod texture_tests {
    use super::*;
    use voltra_assets::AssetPath;

    #[test]
    fn default_sprite_has_no_texture() {
        let sprite = Sprite::default();
        assert!(sprite.texture.is_none());
        assert!(sprite.texture_handle.is_none());
    }

    #[test]
    fn texture_path_round_trips_through_ron_without_the_handle() {
        // No handle is forged here: `Handle::new` is `pub(crate)` to
        // `voltra-assets`, and a handle only ever comes from `Textures::load`.
        // Setting just `texture` and checking `texture_handle` stays `None`
        // through a round trip is the same claim without an API hole.
        let sprite = Sprite {
            texture: Some(AssetPath::new("sprites/hero.png").expect("valid")),
            ..Default::default()
        };

        let text = ron::to_string(&sprite).expect("serialize");
        assert!(
            !text.contains("texture_handle"),
            "handle leaked into RON: {text}"
        );
        let back: Sprite = ron::from_str(&text).expect("deserialize");
        assert_eq!(
            back.texture.as_ref().map(AssetPath::as_str),
            Some("sprites/hero.png")
        );
        assert!(back.texture_handle.is_none(), "handle must not deserialize");
    }

    #[test]
    fn old_ron_without_texture_field_still_loads() {
        let text = "(color:(1.0,1.0,1.0,1.0),sort_order:0)";
        let sprite: Sprite = ron::from_str(text).expect("old scene shape");
        assert!(sprite.texture.is_none());
        assert!(sprite.texture_handle.is_none());
    }

    #[test]
    fn hostile_texture_path_is_rejected_on_deserialize() {
        let hostile =
            r#"(color:(1.0,1.0,1.0,1.0),sort_order:0,texture:Some(Path("../../etc/passwd")))"#;
        assert!(ron::from_str::<Sprite>(hostile).is_err());
    }
}

#[cfg(test)]
mod atlas_tests {
    use super::*;
    use voltra_testkit::scratch_root;

    #[test]
    fn a_default_sprite_has_no_slicing_at_all() {
        let sprite = Sprite::default();

        assert!(sprite.atlas.is_none());
        assert!(sprite.atlas_handle.is_none());
        assert!(sprite.region.is_none());
        assert_eq!(sprite.frame, 0);
        assert!(!sprite.flip_x && !sprite.flip_y);
        assert!(sprite.pixels_per_unit.is_none());
    }

    #[test]
    fn a_scene_written_before_atlases_existed_still_loads() {
        // The whole point of `#[serde(default)]` on every new field: a project
        // on disk must open unchanged and draw exactly as it did.
        let text = "(color:(1.0,1.0,1.0,1.0),sort_order:0)";

        let sprite: Sprite = ron::from_str(text).expect("old scene shape");

        assert_eq!(sprite, Sprite::default());
    }

    #[test]
    fn the_slicing_round_trips_without_its_handle() {
        let sprite = Sprite {
            atlas: Some(AssetPath::new("sprites/coin.atlas.ron").expect("valid")),
            frame: 3,
            flip_x: true,
            pixels_per_unit: Some(16.0),
            region: Some(Frame::new(8, 0, 8, 16)),
            ..Default::default()
        };

        let text = ron::to_string(&sprite).expect("serialize");
        assert!(
            !text.contains("atlas_handle"),
            "handle leaked into RON: {text}"
        );
        let back: Sprite = ron::from_str(&text).expect("deserialize");
        assert_eq!(back, sprite);
        assert!(back.atlas_handle.is_none(), "handle must not deserialize");
    }

    #[test]
    fn setting_an_atlas_resolves_a_handle_and_clearing_it_drops_both() {
        let root = scratch_root();
        std::fs::write(
            root.join("coin.atlas.ron"),
            "(version: 1, grid: Some((cell: (16, 16), columns: 4, rows: 1)))",
        )
        .expect("the fixture writes");
        let mut atlases = Atlases::new(&root);
        let mut sprite = Sprite::default();

        sprite.set_atlas(
            Some(AssetPath::new("coin.atlas.ron").expect("valid")),
            &mut atlases,
        );

        let handle = sprite.atlas_handle.expect("a handle was resolved");
        assert_eq!(atlases.get(handle).len(), 4);

        sprite.set_atlas(None, &mut atlases);

        assert!(sprite.atlas.is_none());
        assert!(sprite.atlas_handle.is_none());
    }
}
