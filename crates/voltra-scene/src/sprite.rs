//! The sprite component: a coloured quad, sized by its entity's transform.

use voltra_assets::{AssetPath, Handle, Textures};
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
}

impl Default for Sprite {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0, 1.0],
            sort_order: 0,
            texture: None,
            texture_handle: None,
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
