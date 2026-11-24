//! A skinned mesh's place in the batch's draw order. Its vertices are the
//! renderer's (deformed on the GPU); the batch only knows when to draw it
//! and whether it can be seen.

use voltra_assets::Handle;
use voltra_render::glam::Vec2;
use voltra_render::Texture;

use super::SpriteBatch;

/// Where a skinned mesh draws among the ranges and the GPU emitters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinnedDraw {
    /// The entity carrying the [`SkinnedSprite`](crate::SkinnedSprite).
    pub entity: voltra_ecs::Entity,
    /// `None` draws against the renderer's white bind group.
    pub texture: Option<Handle<Texture>>,
    /// The light layers it is on.
    pub light_layers: u32,
    /// Drawn before `ranges[before]`, or after every range when it is
    /// `ranges.len()`.
    pub before: usize,
    /// How many GPU emitters were put in before it: what orders it against
    /// an emitter at the same `before`.
    pub after_emitters: usize,
}

impl SpriteBatch {
    /// Puts a skinned mesh's draw here in painter's order — unless its
    /// deformed corners (`drawn`, in the world) all lie outside the view.
    pub fn push_skinned(
        &mut self,
        entity: voltra_ecs::Entity,
        texture: Option<Handle<Texture>>,
        light_layers: u32,
        drawn: &[Vec2],
    ) {
        let min = drawn.iter().copied().fold(Vec2::INFINITY, Vec2::min);
        let max = drawn.iter().copied().fold(Vec2::NEG_INFINITY, Vec2::max);
        let view = crate::sprite::sheets::Sheets {
            visible: self.visible,
            ..Default::default()
        };
        if drawn.is_empty() || !view.sees(min, max) {
            return;
        }
        self.skinned.push(SkinnedDraw {
            entity,
            texture,
            light_layers,
            before: self.ranges.len(),
            after_emitters: self.gpu_emitters.len(),
        });
        self.sealed = Some(self.ranges.len());
    }
}
