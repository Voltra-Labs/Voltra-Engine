//! How the frame's shots are ordered and pruned: the textures a 2D world
//! shows, producers before consumers, viewports that share a pixel.

use std::collections::HashSet;

use voltra_assets::Handle;
use voltra_ecs::{Entity, World};
use voltra_render::Texture;

use super::{AnyShot, CameraTarget, Viewport};
use crate::sprite::Sprite2D;
use crate::ui::UiRect;

/// The render textures the 2D world shows: those its sprites name, the
/// HUD's left out — it is drawn after every shot.
pub(super) fn shown_by_2d(world: &World) -> HashSet<Handle<Texture>> {
    let targets: HashSet<Handle<Texture>> = world
        .query::<CameraTarget>()
        .filter_map(|(_, t)| t.handle)
        .collect();
    if targets.is_empty() {
        return targets;
    }
    world
        .query::<Sprite2D>()
        .filter(|(e, _)| world.get::<UiRect>(*e).is_none())
        .filter_map(|(_, sprite)| sprite.texture_handle)
        .filter(|h| targets.contains(h))
        .collect()
}

/// `shots` reordered so each comes after the shots filling the textures
/// it `shows`, keeping their order otherwise (Kahn's, always taking the
/// first ready). Where none is ready — a cycle — the first left goes, and
/// is returned too.
pub(super) fn producers_first(
    shots: Vec<AnyShot>,
    shows: impl Fn(&AnyShot) -> Vec<Handle<Texture>>,
) -> (Vec<AnyShot>, Vec<Entity>) {
    let needs: Vec<Vec<Handle<Texture>>> = shots.iter().map(&shows).collect();
    let mut left: Vec<usize> = (0..shots.len()).collect();
    let mut order = Vec::with_capacity(shots.len());
    let mut cycle = Vec::new();
    while !left.is_empty() {
        let ready = left.iter().position(|&i| {
            needs[i].iter().all(|t| {
                // Every shot still to draw that fills `t` is a producer not
                // yet drawn.
                !left
                    .iter()
                    .any(|&j| j != i && shots[j].target() == Some(*t))
            })
        });
        let at = ready.unwrap_or_else(|| {
            cycle.push(shots[left[0]].entity());
            0
        });
        order.push(shots[left.remove(at)]);
    }
    (order, cycle)
}

/// Whether two viewports share any area.
pub(super) fn overlaps(a: Viewport, b: Viewport) -> bool {
    let (a, b) = (a.clamped(), b.clamped());
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}
