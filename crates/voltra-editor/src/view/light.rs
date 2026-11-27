//! The selected light's reach, drawn in the scene view.
//!
//! A light has no sprite: without an outline its radius is a guess, and a
//! spot light's aim is invisible until the frame is lit. Unity draws a
//! selected `Light2D`'s radius and cone; so does this, for the selected
//! light only, as the camera's rectangle is.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_render::LineBatch;
use voltra_scene::hierarchy::world_matrix;
use voltra_scene::PointLight2D;

/// The light's own warm white: not a gizmo axis, the camera's amber, the
/// caster's violet or the physics overlay's green and magenta.
const COLOR: [f32; 4] = [1.0, 0.95, 0.7, 0.8];

/// Outline thickness, in pixels. Matches the other overlays.
const WIDTH: f32 = 1.5;

/// Segments in a whole circle; an arc gets its share.
const SEGMENTS: usize = 48;

/// Pushes `entity`'s light outline into `lines`, if it is a light: the
/// circle it reaches, or for a spot light the arc of its cone and the two
/// edges from the centre.
pub fn draw(world: &World, entity: Entity, lines: &mut LineBatch) {
    let Some(light) = world.get::<PointLight2D>(entity) else {
        return;
    };
    let placed = light.placed(world_matrix(world, entity));
    // Drawn exactly when the renderer draws it: a light that gives nothing
    // (no radius, a cone that shines nowhere, a value that is not a number)
    // has no reach to outline.
    if !placed.gives_light() {
        return;
    }
    let (centre, radius) = (placed.position, placed.radius);
    let cone = placed
        .cone
        .filter(|cone| cone.angle < std::f32::consts::TAU)
        .and_then(|cone| Some((cone.direction.try_normalize()?, cone.angle)));

    let (start, sweep) = match cone {
        Some((direction, angle)) => (direction.to_angle() - angle * 0.5, angle),
        None => (0.0, std::f32::consts::TAU),
    };
    let steps = ((SEGMENTS as f32 * sweep / std::f32::consts::TAU).ceil() as usize).max(1);
    let at = |i: usize| centre + Vec2::from_angle(start + sweep * i as f32 / steps as f32) * radius;
    for i in 0..steps {
        lines.push(at(i), at(i + 1), WIDTH, COLOR);
    }
    if cone.is_some() {
        lines.push(centre, at(0), WIDTH, COLOR);
        lines.push(centre, at(steps), WIDTH, COLOR);
    }
}

#[cfg(test)]
mod tests {
    use voltra_scene::{LightCone, Transform2D};

    use super::*;

    fn light(world: &mut World, cone: Option<LightCone>) -> Entity {
        let entity = world.spawn();
        world.insert(entity, Transform2D::default());
        world.insert(
            entity,
            PointLight2D {
                cone,
                ..PointLight2D::default()
            },
        );
        entity
    }

    #[test]
    fn a_point_light_draws_a_circle_and_a_spot_light_an_arc_and_two_edges() {
        let mut world = World::new();
        let point = light(&mut world, None);
        let spot = light(
            &mut world,
            Some(LightCone {
                angle: std::f32::consts::FRAC_PI_2,
                softness: 0.0,
            }),
        );
        let mut lines = LineBatch::default();
        draw(&world, point, &mut lines);
        let circle = lines.len();
        lines.clear();
        draw(&world, spot, &mut lines);
        let per_line = circle / SEGMENTS;
        assert_eq!(
            lines.len(),
            per_line * (SEGMENTS / 4 + 2),
            "a quarter arc and two edges"
        );
    }

    #[test]
    fn a_light_that_gives_nothing_draws_nothing() {
        let mut world = World::new();
        let squashed = light(&mut world, Some(LightCone::default()));
        // Scaled to nothing along x: its cone points nowhere.
        if let Some(transform) = world.get_mut::<Transform2D>(squashed) {
            transform.scale = Vec2::new(0.0, 1.0);
        }
        let mut lines = LineBatch::default();
        draw(&world, squashed, &mut lines);
        assert!(lines.is_empty());
    }

    #[test]
    fn anything_but_a_light_draws_nothing() {
        let mut world = World::new();
        let plain = world.spawn();
        world.insert(plain, Transform2D::default());
        let mut lines = LineBatch::default();
        draw(&world, plain, &mut lines);
        assert!(lines.is_empty());
    }
}
