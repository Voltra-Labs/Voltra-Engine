//! Every bone, drawn in the scene view as the familiar kite: wide near its
//! origin, to a point at its tip.
//!
//! A bone has no sprite of its own, so without this a skeleton is
//! invisible and its bones cannot be clicked. Godot and Unity both draw
//! bones over the art while editing; so does this, the selected one lit.
//! A click near a bone selects it before the sprites hung on it
//! ([`crate::picking`]).

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_render::LineBatch;
use voltra_scene::hierarchy::WorldTransforms;
use voltra_scene::skeleton::placed_bones;

/// A cool cyan: not a gizmo axis, the camera's amber, the caster's violet,
/// the light's white or the physics overlay's green and magenta.
const COLOR: [f32; 4] = [0.35, 0.85, 1.0, 0.8];
/// The selected bone.
const SELECTED: [f32; 4] = [1.0, 0.85, 0.3, 1.0];
/// Outline thickness, in pixels. Matches the other overlays.
const WIDTH: f32 = 1.5;

/// Pushes every bone in `world` into `lines`, `selected` lit. `per_pixel`
/// is world units per screen pixel, which caps how wide a kite is drawn.
pub fn draw(world: &World, selected: Option<Entity>, per_pixel: f32, lines: &mut LineBatch) {
    let transforms = WorldTransforms::from_world(world);
    for bone in placed_bones(world, &transforms) {
        let along = bone.end - bone.start;
        let length = along.length();
        if !(length.is_finite() && length > 0.0) {
            continue;
        }
        let color = if Some(bone.entity) == selected {
            SELECTED
        } else {
            COLOR
        };
        let direction = along / length;
        let half = (length * 0.12).min(8.0 * per_pixel.max(0.0));
        let waist = bone.start + direction * (length * 0.2);
        let side = direction.perp() * half;
        let kite = [bone.start, waist + side, bone.end, waist - side];
        for i in 0..4 {
            lines.push(kite[i], kite[(i + 1) % 4], WIDTH, color);
        }
        // The joint: a small square at the origin, where a child is hung.
        let r = half * 0.5;
        let corners = [
            bone.start + Vec2::new(-r, -r),
            bone.start + Vec2::new(r, -r),
            bone.start + Vec2::new(r, r),
            bone.start + Vec2::new(-r, r),
        ];
        for i in 0..4 {
            lines.push(corners[i], corners[(i + 1) % 4], WIDTH, color);
        }
    }
}
