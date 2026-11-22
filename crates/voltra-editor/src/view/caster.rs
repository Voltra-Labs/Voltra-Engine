//! The selected shadow caster's outline, drawn in the scene view.
//!
//! A caster shaped by its collider sits under the physics overlay's
//! outline when that is on, but a polygon of its own is invisible
//! otherwise. Drawn for the selected entity only, as the camera's rectangle
//! is.

#[cfg(test)]
use voltra_ecs::{Entity, World};
use voltra_render::{LineBatch, Occluder};
#[cfg(test)]
use voltra_scene::shadow_caster;

/// Cool violet: not a gizmo axis, the camera's amber or the physics
/// overlay's green and magenta.
const COLOR: [f32; 4] = [0.6, 0.45, 1.0, 0.9];

/// Outline thickness, in pixels. Matches the other overlays.
const WIDTH: f32 = 1.5;

/// Pushes `outlines` — a caster's, from
/// [`outlines_of`](shadow_caster::outlines_of) — into `lines`.
pub fn draw(outlines: &[Occluder], lines: &mut LineBatch) {
    for outline in outlines {
        let points = &outline.points;
        let edges = if outline.closed {
            points.len()
        } else {
            points.len().saturating_sub(1)
        };
        for i in 0..edges {
            lines.push(points[i], points[(i + 1) % points.len()], WIDTH, COLOR);
        }
    }
}

#[cfg(test)]
mod tests {
    use voltra_render::glam::Vec2;
    use voltra_scene::{CasterShape, ShadowCaster2D, Transform};

    use super::*;

    fn caster(world: &mut World, closed: bool) -> Entity {
        let entity = world.spawn();
        world.insert(entity, Transform::default());
        world.insert(
            entity,
            ShadowCaster2D {
                shape: CasterShape::Polygon {
                    points: vec![Vec2::ZERO, Vec2::X, Vec2::Y],
                    closed,
                },
                self_shadow: false,
                ..ShadowCaster2D::default()
            },
        );
        entity
    }

    #[test]
    fn a_closed_outline_draws_every_edge_and_an_open_line_one_fewer() {
        let mut world = World::new();
        let (outline, line) = (caster(&mut world, true), caster(&mut world, false));
        let mut lines = LineBatch::default();
        draw(
            &shadow_caster::outlines_of(&world, outline, None),
            &mut lines,
        );
        let closed = lines.len();
        lines.clear();
        draw(&shadow_caster::outlines_of(&world, line, None), &mut lines);
        assert_eq!(closed, 3 * (lines.len() / 2), "three edges against two");
    }

    #[test]
    fn an_entity_that_casts_nothing_draws_nothing() {
        let mut world = World::new();
        let plain = world.spawn();
        world.insert(plain, Transform::default());
        let mut lines = LineBatch::default();
        draw(&shadow_caster::outlines_of(&world, plain, None), &mut lines);
        assert!(lines.is_empty());
    }
}
