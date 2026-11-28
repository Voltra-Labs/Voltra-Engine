use voltra_render::glam::{Affine3A, Quat, Vec3};
use voltra_render::render3d::{Aabb3, View3D, MAX_DIRECTIONAL, MAX_PUNCTUAL};

use super::*;
use crate::{
    DirectionalLight3D, Environment3D, Fog3D, PointLight3D, SceneId, SpotLight3D, Transform3D,
};

fn spawn(world: &mut World) -> Entity {
    let e = world.spawn();
    world.insert(e, SceneId::new());
    e
}

/// From 10 m back on +Z, looking down −Z at the origin.
fn view() -> View3D {
    View3D::perspective(
        Affine3A::from_translation(Vec3::new(0.0, 0.0, 10.0)),
        1.0,
        1.0,
        0.1,
        None,
    )
}

fn lamp(world: &mut World, at: Vec3, light: PointLight3D) -> Entity {
    let e = spawn(world);
    world.insert(e, Transform3D::from_translation(at));
    world.insert(e, light);
    e
}

fn unit_box(at: Vec3) -> Aabb3 {
    Aabb3::new(at - Vec3::splat(0.5), at + Vec3::splat(0.5))
}

#[test]
fn every_directional_light_shines_down_its_entity_minus_z() {
    let mut world = World::new();
    let camera = spawn(&mut world);
    world.insert(camera, Camera3D::default());
    let none = gather(&world, camera, &view());
    assert!(none.lighting.directional.is_empty());
    assert_eq!(
        none.lighting.ambient,
        [Environment3D::DEFAULT_AMBIENT; 3],
        "no environment: the default's"
    );
    let sun = spawn(&mut world);
    world.insert(
        sun,
        Transform3D::default().with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    );
    world.insert(
        sun,
        DirectionalLight3D {
            color: [1.0, 0.5, 0.25],
            energy: 3.0,
            ..DirectionalLight3D::default()
        },
    );
    let second = spawn(&mut world);
    world.insert(second, DirectionalLight3D::default());
    let lit = gather(&world, camera, &view());
    assert_eq!(lit.directional, [sun, second], "both, in entity order");
    let light = lit.lighting.directional[0];
    // Turned −90° about X, −Z points down.
    assert!(
        light.direction.abs_diff_eq(Vec3::NEG_Y, 1e-6),
        "{}",
        light.direction
    );
    assert_eq!((light.color, light.energy), ([1.0, 0.5, 0.25], 3.0));
    // A broken one is left out and named.
    world.insert(
        sun,
        DirectionalLight3D {
            energy: f32::NAN,
            ..DirectionalLight3D::default()
        },
    );
    let skipped = gather(&world, camera, &view());
    assert_eq!(skipped.directional, [second]);
    assert_eq!(skipped.broken.len(), 1);
    assert_eq!(skipped.broken[0].0, sun);
    // Despawned: none left over.
    world.despawn(second);
    world.despawn(sun);
    assert!(gather(&world, camera, &view())
        .lighting
        .directional
        .is_empty());
}

#[test]
fn past_four_suns_the_dimmest_are_dropped_and_counted() {
    let mut world = World::new();
    let camera = spawn(&mut world);
    let energies = [1.0, 5.0, 0.5, 3.0, 4.0, 2.0];
    let suns: Vec<Entity> = energies
        .iter()
        .map(|&energy| {
            let e = spawn(&mut world);
            world.insert(
                e,
                DirectionalLight3D {
                    energy,
                    ..DirectionalLight3D::default()
                },
            );
            e
        })
        .collect();
    let lit = gather(&world, camera, &view());
    assert_eq!(lit.lighting.directional.len(), MAX_DIRECTIONAL);
    assert_eq!(lit.dropped_directional, 2);
    // 5, 3, 4, 2 kept — in entity order; 1 and 0.5 dropped.
    assert_eq!(lit.directional, [suns[1], suns[3], suns[4], suns[5]]);
}

#[test]
fn point_and_spot_lights_are_placed_and_culled_by_their_reach() {
    let mut world = World::new();
    let camera = spawn(&mut world);
    let near = lamp(
        &mut world,
        Vec3::new(1.0, 2.0, 0.0),
        PointLight3D::default(),
    );
    // Far behind the camera, its reach not touching the view.
    lamp(
        &mut world,
        Vec3::new(0.0, 0.0, 40.0),
        PointLight3D::default(),
    );
    // Behind, but reaching into the view.
    let reaching = lamp(
        &mut world,
        Vec3::new(0.0, 0.0, 12.0),
        PointLight3D {
            range: 5.0,
            ..PointLight3D::default()
        },
    );
    // Lights nothing: quietly out.
    lamp(
        &mut world,
        Vec3::ZERO,
        PointLight3D {
            energy: 0.0,
            ..PointLight3D::default()
        },
    );
    let spot = spawn(&mut world);
    world.insert(
        spot,
        Transform3D::from_translation(Vec3::new(0.0, 3.0, 0.0))
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    );
    world.insert(spot, SpotLight3D::default());
    let lit = gather(&world, camera, &view());
    let entities: Vec<u32> = lit.punctual.iter().map(|r| r.entity).collect();
    assert_eq!(entities, [near.index(), reaching.index(), spot.index()]);
    assert_eq!(lit.lighting.punctual[0].position, Vec3::new(1.0, 2.0, 0.0));
    let cone = lit.lighting.punctual[2].spot.expect("a spot");
    assert!(cone.direction.abs_diff_eq(Vec3::NEG_Y, 1e-6));
    assert!((cone.outer - 45f32.to_radians()).abs() < 1e-6);
    assert!(lit.broken.is_empty());
    world.insert(
        near,
        PointLight3D {
            range: f32::NAN,
            ..PointLight3D::default()
        },
    );
    let broken = gather(&world, camera, &view());
    assert_eq!(broken.lighting.punctual.len(), 2);
    assert_eq!(broken.broken[0].0, near);
    world.despawn(spot);
    world.despawn(reaching);
    assert_eq!(gather(&world, camera, &view()).lighting.punctual.len(), 0);
}

#[test]
fn past_128_lights_in_view_those_furthest_from_the_eye_are_dropped() {
    let mut world = World::new();
    let camera = spawn(&mut world);
    // 130 in a line from the eye: the two furthest go.
    let all: Vec<Entity> = (0..130)
        .map(|i| {
            lamp(
                &mut world,
                Vec3::new(0.0, 0.0, 5.0 - i as f32),
                PointLight3D {
                    range: 1.0,
                    ..PointLight3D::default()
                },
            )
        })
        .collect();
    let lit = gather(&world, camera, &view());
    assert_eq!(lit.lighting.punctual.len(), MAX_PUNCTUAL);
    assert_eq!(lit.dropped_punctual, 2);
    let kept: Vec<u32> = lit.punctual.iter().map(|r| r.entity).collect();
    assert!(!kept.contains(&all[129].index()) && !kept.contains(&all[128].index()));
    assert!(kept.windows(2).all(|w| w[0] < w[1]), "in entity order");
}

#[test]
fn nine_lights_on_one_mesh_keep_the_eight_brightest() {
    let mut world = World::new();
    let camera = spawn(&mut world);
    // Nine about a box at the origin, 2 m off, energies 1 to 9.
    let lamps: Vec<Entity> = (0..9)
        .map(|i| {
            let angle = i as f32 / 9.0 * std::f32::consts::TAU;
            lamp(
                &mut world,
                Vec3::new(angle.cos(), angle.sin(), 0.0) * 2.5,
                PointLight3D {
                    energy: 1.0 + i as f32,
                    range: 10.0,
                    ..PointLight3D::default()
                },
            )
        })
        .collect();
    let lit = gather(&world, camera, &view());
    let picked = pick(
        &lit.punctual,
        &lit.directional_layers,
        &unit_box(Vec3::ZERO),
        crate::light::default_light_layers(),
    );
    assert_eq!(picked.reached, 9);
    let chosen: Vec<u32> = picked
        .list
        .punctual()
        .map(|i| lit.punctual[usize::from(i)].entity)
        .collect();
    assert_eq!(chosen.len(), 8);
    assert!(!chosen.contains(&lamps[0].index()), "the dimmest dropped");
    // Brightest first.
    assert_eq!(chosen[0], lamps[8].index());
}

#[test]
fn the_choice_holds_as_the_camera_moves_and_ties_go_to_the_lower_entity() {
    let mut world = World::new();
    let camera = spawn(&mut world);
    // Ten equal lights 3 m out along the axes, two to a place: all ten
    // estimated alike, a tie for the eighth place.
    let places = [Vec3::X, Vec3::NEG_X, Vec3::Z, Vec3::NEG_Z, Vec3::Y];
    let lamps: Vec<Entity> = (0..10)
        .map(|i| {
            lamp(
                &mut world,
                places[i % places.len()] * 3.0,
                PointLight3D {
                    range: 10.0,
                    ..PointLight3D::default()
                },
            )
        })
        .collect();
    let bounds = unit_box(Vec3::ZERO);
    let chosen_from = |eye: Vec3| {
        let view = View3D::perspective(
            Transform3D::from_translation(eye)
                .looking_at(Vec3::ZERO, Vec3::Y)
                .matrix(),
            1.2,
            1.0,
            0.1,
            None,
        );
        let lit = gather(&world, camera, &view);
        let picked = pick(&lit.punctual, &lit.directional_layers, &bounds, 1);
        let mut chosen: Vec<u32> = picked
            .list
            .punctual()
            .map(|i| lit.punctual[usize::from(i)].entity)
            .collect();
        chosen.sort_unstable();
        chosen
    };
    let first = chosen_from(Vec3::new(0.0, 2.0, 12.0));
    // The eight lowest entities win the tie.
    let lowest: Vec<u32> = lamps[..8].iter().map(|e| e.index()).collect();
    assert_eq!(first, lowest);
    for eye in [
        Vec3::new(5.0, 1.0, 8.0),
        Vec3::new(-7.0, 3.0, 2.0),
        Vec3::new(0.5, 9.0, 0.5),
    ] {
        assert_eq!(chosen_from(eye), first, "from {eye}");
    }
}

#[test]
fn layers_exclude_and_reach_and_cones_bound_the_choice() {
    let mut world = World::new();
    let camera = spawn(&mut world);
    let sun = spawn(&mut world);
    world.insert(
        sun,
        DirectionalLight3D {
            layers: 0b10,
            ..DirectionalLight3D::default()
        },
    );
    let every = spawn(&mut world);
    world.insert(every, DirectionalLight3D::default());
    lamp(
        &mut world,
        Vec3::new(1.5, 0.0, 0.0),
        PointLight3D {
            layers: 0b10,
            ..PointLight3D::default()
        },
    );
    // Out of reach of the box: 6 m off, range 5.
    lamp(
        &mut world,
        Vec3::new(-6.6, 0.0, 0.0),
        PointLight3D::default(),
    );
    // A spot above, pointing away (up): its cone misses the box.
    let away = spawn(&mut world);
    world.insert(
        away,
        Transform3D::from_translation(Vec3::new(0.0, 2.0, 0.0))
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
    );
    world.insert(away, SpotLight3D::default());
    // One pointing down at it.
    let down = spawn(&mut world);
    world.insert(
        down,
        Transform3D::from_translation(Vec3::new(0.0, 2.0, 0.0))
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    );
    world.insert(down, SpotLight3D::default());
    let lit = gather(&world, camera, &view());
    let bounds = unit_box(Vec3::ZERO);
    let first = pick(&lit.punctual, &lit.directional_layers, &bounds, 0b01);
    assert_eq!(first.list.directional(), 0b10, "the second sun alone");
    let chosen: Vec<u32> = first
        .list
        .punctual()
        .map(|i| lit.punctual[usize::from(i)].entity)
        .collect();
    assert_eq!(chosen, [down.index()]);
    let second = pick(&lit.punctual, &lit.directional_layers, &bounds, 0b10);
    assert_eq!(second.list.directional(), 0b11);
    assert_eq!(
        second.list.punctual().count(),
        2,
        "the layer-2 lamp and the spot"
    );
    let none = pick(&lit.punctual, &lit.directional_layers, &bounds, 0);
    assert_eq!(none.list.directional(), 0);
    assert_eq!(none.list.punctual().count(), 0);
}

#[test]
fn a_camera_s_own_environment_wins_over_the_world_s() {
    let mut world = World::new();
    let camera = spawn(&mut world);
    world.insert(camera, Camera3D::default());
    let other = spawn(&mut world);
    world.insert(other, Camera3D::default());
    world.insert(other, Environment3D::with_ambient([9.0; 3]));
    let a = spawn(&mut world);
    world.insert(
        a,
        Environment3D {
            ambient_energy: 2.0,
            fog: Some(Fog3D::default()),
            background: crate::Background3D::DAYLIGHT,
            ..Environment3D::with_ambient([0.3; 3])
        },
    );
    let b = spawn(&mut world);
    world.insert(b, Environment3D::with_ambient([0.6; 3]));
    let shared = gather(&world, camera, &view());
    assert_eq!(
        shared.lighting.ambient, [0.6; 3],
        "the world's, lowest index, times 2"
    );
    assert!(shared.lighting.fog.is_some() && shared.lighting.sky.is_some());
    assert_eq!(
        (shared.environment, shared.unused_environments),
        (Some(a), 1),
        "another camera's is not the world's"
    );
    world.insert(camera, Environment3D::with_ambient([0.0; 3]));
    let own = gather(&world, camera, &view());
    assert_eq!(own.lighting.ambient, [0.0; 3]);
    assert!(own.lighting.fog.is_none() && own.lighting.sky.is_none());
    assert_eq!(
        (own.environment, own.unused_environments),
        (Some(camera), 0)
    );
}
