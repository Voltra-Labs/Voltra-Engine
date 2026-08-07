//! The sprite component: a coloured quad, sized by its entity's transform.

use voltra_ecs::Entity;

/// A coloured quad. Its size comes from the entity's [`Transform`].
///
/// [`Transform`]: crate::transform::Transform
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
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
}

impl Default for Sprite {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0, 1.0],
            sort_order: 0,
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
