//! What lights a 3D camera's view: the world's directional light and the
//! environment's ambient, gathered for the render side.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec3;
use voltra_render::render3d::{DirectionalLight, Lighting3D};

use crate::directional_light3d::DirectionalLight3D;
use crate::environment3d::Environment3D;
use crate::Camera3D;

/// A camera's light, and what was left out of it — for the frame to warn
/// about once.
#[derive(Debug, Clone, PartialEq)]
pub struct Gathered3D {
    /// What the view is lit by.
    pub lighting: Lighting3D,
    /// The directional light used, if any.
    pub light: Option<Entity>,
    /// Directional lights beyond the one used (stage 139 lights them all).
    pub unused_lights: usize,
    /// The environment used, if any: the camera's own, else the world's.
    pub environment: Option<Entity>,
    /// World environments beyond the one used.
    pub unused_environments: usize,
}

/// The light `camera` sees `world` by: the directional light on the lowest
/// entity index, shining down its entity's −Z, and the ambient of the
/// camera's own [`Environment3D`] — else the world's, an `Environment3D`
/// on an entity with no camera, the lowest index — else the default's.
///
/// A light whose colour or energy cannot be drawn, or whose entity's
/// transform has no direction, is skipped for the next.
pub fn gather(world: &World, camera: Entity) -> Gathered3D {
    let mut lights: Vec<(Entity, DirectionalLight3D)> = world
        .query::<DirectionalLight3D>()
        .map(|(e, light)| (e, *light))
        .collect();
    lights.sort_by_key(|(e, _)| e.index());
    let mut used = None;
    for (entity, light) in &lights {
        if light.problem().is_some() {
            continue;
        }
        let direction = crate::hierarchy::world_matrix_3d(world, *entity)
            .transform_vector3(Vec3::NEG_Z)
            .normalize_or_zero();
        if direction == Vec3::ZERO {
            continue;
        }
        used = Some((
            *entity,
            DirectionalLight {
                direction,
                color: light.color,
                energy: light.energy,
            },
        ));
        break;
    }
    let own = world.get::<Environment3D>(camera).map(|env| (camera, *env));
    let mut shared: Vec<(Entity, Environment3D)> = world
        .query::<Environment3D>()
        .filter(|(e, _)| world.get::<Camera3D>(*e).is_none())
        .map(|(e, env)| (e, *env))
        .collect();
    shared.sort_by_key(|(e, _)| e.index());
    let environment = own.or_else(|| shared.first().copied());
    let ambient = environment.map_or_else(|| Environment3D::default().ambient, |(_, e)| e.ambient);
    Gathered3D {
        lighting: Lighting3D {
            directional: used.map(|(_, light)| light),
            ambient,
        },
        light: used.map(|(e, _)| e),
        unused_lights: lights.len().saturating_sub(usize::from(used.is_some())),
        environment: environment.map(|(e, _)| e),
        unused_environments: if own.is_some() {
            0
        } else {
            shared.len().saturating_sub(1)
        },
    }
}

#[cfg(test)]
mod tests {
    use voltra_render::glam::Quat;

    use super::*;
    use crate::{SceneId, Transform3D};

    fn spawn(world: &mut World) -> Entity {
        let e = world.spawn();
        world.insert(e, SceneId::new());
        e
    }

    #[test]
    fn the_first_light_shines_down_its_entity_minus_z() {
        let mut world = World::new();
        let camera = spawn(&mut world);
        world.insert(camera, Camera3D::default());
        let none = gather(&world, camera);
        assert_eq!(none.lighting.directional, None);
        assert_eq!(
            none.lighting.ambient,
            [Environment3D::DEFAULT_AMBIENT; 3],
            "no environment: the default's"
        );
        let sun = spawn(&mut world);
        world.insert(
            sun,
            Transform3D::default()
                .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
        );
        world.insert(
            sun,
            DirectionalLight3D {
                color: [1.0, 0.5, 0.25],
                energy: 3.0,
            },
        );
        let second = spawn(&mut world);
        world.insert(second, DirectionalLight3D::default());
        let lit = gather(&world, camera);
        let light = lit.lighting.directional.expect("a light");
        // Turned −90° about X, −Z points down.
        assert!(
            light.direction.abs_diff_eq(Vec3::NEG_Y, 1e-6),
            "{}",
            light.direction
        );
        assert_eq!((light.color, light.energy), ([1.0, 0.5, 0.25], 3.0));
        assert_eq!((lit.light, lit.unused_lights), (Some(sun), 1));
        // A broken one is skipped for the next.
        world.insert(
            sun,
            DirectionalLight3D {
                energy: f32::NAN,
                ..DirectionalLight3D::default()
            },
        );
        assert_eq!(gather(&world, camera).light, Some(second));
        // Despawned: none left over.
        world.despawn(second);
        world.despawn(sun);
        assert_eq!(gather(&world, camera).lighting.directional, None);
    }

    #[test]
    fn a_camera_s_own_environment_wins_over_the_world_s() {
        let mut world = World::new();
        let camera = spawn(&mut world);
        world.insert(camera, Camera3D::default());
        let other = spawn(&mut world);
        world.insert(other, Camera3D::default());
        world.insert(other, Environment3D { ambient: [9.0; 3] });
        let a = spawn(&mut world);
        world.insert(a, Environment3D { ambient: [0.3; 3] });
        let b = spawn(&mut world);
        world.insert(b, Environment3D { ambient: [0.6; 3] });
        let shared = gather(&world, camera);
        assert_eq!(
            shared.lighting.ambient, [0.3; 3],
            "the world's, lowest index"
        );
        assert_eq!(
            (shared.environment, shared.unused_environments),
            (Some(a), 1),
            "another camera's is not the world's"
        );
        world.insert(camera, Environment3D { ambient: [0.0; 3] });
        let own = gather(&world, camera);
        assert_eq!(own.lighting.ambient, [0.0; 3]);
        assert_eq!(
            (own.environment, own.unused_environments),
            (Some(camera), 0)
        );
    }
}
