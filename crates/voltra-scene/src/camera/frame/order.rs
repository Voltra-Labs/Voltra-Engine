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
/// first ready). Where none is ready — a cycle — the first shot left whose
/// own texture another shot left waits for goes: a producer inside the
/// cycle, not a mere consumer of it. It is returned too.
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
            let waited_for = |&i: &usize| {
                let mine = shots[i].target();
                left.iter()
                    .any(|&j| j != i && needs[j].iter().any(|t| Some(*t) == mine))
            };
            // Stuck means every shot left waits on one, so one is waited for.
            let at = left.iter().position(waited_for).unwrap_or(0);
            cycle.push(shots[left[at]].entity());
            at
        });
        order.push(shots[left.remove(at)]);
    }
    (order, cycle)
}

/// Whether two viewports share a pixel of a `screen` of that many: in the
/// whole-pixel rectangles they are drawn in, so viewports that adjoin
/// never overlap by a rounding.
pub(super) fn overlaps(a: Viewport, b: Viewport, (width, height): (u32, u32)) -> bool {
    let [ax, ay, aw, ah] = a.pixels(width.max(1), height.max(1));
    let [bx, by, bw, bh] = b.pixels(width.max(1), height.max(1));
    ax < bx + bw && bx < ax + aw && ay < by + bh && by < ay + ah
}

/// `shots`, the screen's in draw order, less each wholly under a later
/// full-screen shot of its own dimension with none of the other between.
pub(super) fn prune(shots: Vec<AnyShot>) -> Vec<AnyShot> {
    let hidden: Vec<bool> = (0..shots.len())
        .map(|i| {
            let kind = shots[i].is_3d();
            shots[i + 1..]
                .iter()
                .take_while(|later| later.is_3d() == kind)
                .any(|later| later.viewport().clamped().is_full())
        })
        .collect();
    shots
        .into_iter()
        .zip(hidden)
        .filter(|(_, hidden)| !hidden)
        .map(|(shot, _)| shot)
        .collect()
}
