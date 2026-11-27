use voltra_render::glam::Vec2;

use super::*;
use crate::{hierarchy, Transform};

fn lit_camera(world: &mut World, ambient: Option<[f32; 3]>) {
    let camera = world.spawn();
    world.insert(camera, Transform::default());
    world.insert(
        camera,
        Camera {
            ambient,
            ..Camera::default()
        },
    );
}

fn lamp(world: &mut World, at: Vec2) -> voltra_ecs::Entity {
    let lamp = world.spawn();
    world.insert(lamp, Transform::from_translation(at));
    world.insert(lamp, PointLight2D::default());
    lamp
}

const VIEW: (Vec2, Vec2) = (Vec2::new(-10.0, -10.0), Vec2::new(10.0, 10.0));

#[test]
fn without_an_ambient_the_frame_is_unlit_whatever_lights_there_are() {
    let mut world = World::new();
    lit_camera(&mut world, None);
    lamp(&mut world, Vec2::ZERO);
    assert_eq!(
        gather(&world, VIEW.0, VIEW.1, None, None),
        Lighting::default()
    );
}

#[test]
fn lights_in_reach_of_the_view_are_gathered_and_the_rest_left_out() {
    let mut world = World::new();
    lit_camera(&mut world, Some([0.1; 3]));
    lamp(&mut world, Vec2::new(1.0, 2.0));
    // Off the view's right edge, but its radius of 3 reaches back in.
    lamp(&mut world, Vec2::new(12.0, 0.0));
    lamp(&mut world, Vec2::new(100.0, 0.0));
    let lighting = gather(&world, VIEW.0, VIEW.1, None, None);
    assert_eq!(lighting.ambient, Some([0.1; 3]));
    let at: Vec<Vec2> = lighting.lights.iter().map(|l| l.position).collect();
    assert_eq!(at.len(), 2, "{at:?}");
    assert!(at.contains(&Vec2::new(1.0, 2.0)) && at.contains(&Vec2::new(12.0, 0.0)));
}

#[test]
fn a_spot_light_aimed_away_from_the_view_is_left_out() {
    let mut world = World::new();
    lit_camera(&mut world, Some([0.1; 3]));
    // Just off the view's right edge, its disc reaching back in.
    let toward = lamp(&mut world, Vec2::new(12.0, 0.0));
    let away = lamp(&mut world, Vec2::new(12.0, 5.0));
    let dark = lamp(&mut world, Vec2::ZERO);
    for entity in [toward, away] {
        let light = world.get_mut::<PointLight2D>(entity).expect("a light");
        light.cone = Some(LightCone::default());
    }
    // Turned half a turn: shining left, back into the view.
    world.insert(
        toward,
        Transform {
            rotation: std::f32::consts::PI,
            ..Transform::from_translation(Vec2::new(12.0, 0.0))
        },
    );
    world
        .get_mut::<PointLight2D>(dark)
        .expect("a light")
        .intensity = 0.0;
    let at: Vec<Vec2> = gather(&world, VIEW.0, VIEW.1, None, None)
        .lights
        .iter()
        .map(|l| l.position)
        .collect();
    assert_eq!(at.len(), 1, "{at:?}");
    assert!((at[0] - Vec2::new(12.0, 0.0)).length() < 1e-4, "{at:?}");
}

#[test]
fn a_light_follows_its_parent_and_scales_with_it() {
    let mut world = World::new();
    lit_camera(&mut world, Some([0.1; 3]));
    let parent = world.spawn();
    world.insert(
        parent,
        Transform::from_translation(Vec2::new(4.0, 0.0)).with_scale(Vec2::splat(2.0)),
    );
    world.insert(parent, crate::SceneId::new());
    let child = lamp(&mut world, Vec2::new(1.0, 0.0));
    hierarchy::set_parent(&mut world, child, parent).expect("no cycle");
    let light = gather(&world, VIEW.0, VIEW.1, None, None).lights[0];
    assert!(
        (light.position - Vec2::new(6.0, 0.0)).length() < 1e-5,
        "{light:?}"
    );
    assert!((light.radius - 6.0).abs() < 1e-5);
}

#[test]
fn a_light_round_trips_through_ron_and_fills_what_is_left_out() {
    let light: PointLight2D = ron::from_str("(radius: 5.0)").expect("reads");
    assert_eq!(light.radius, 5.0);
    assert_eq!(light.falloff, 2.0);
    let text = ron::to_string(&light).expect("writes");
    assert_eq!(ron::from_str::<PointLight2D>(&text).expect("reads"), light);
}

#[test]
fn casters_are_gathered_only_for_a_light_that_casts_shadows() {
    let mut world = World::new();
    lit_camera(&mut world, Some([0.1; 3]));
    let lamp = lamp(&mut world, Vec2::ZERO);
    let wall = world.spawn();
    world.insert(wall, Transform::from_translation(Vec2::new(2.0, 0.0)));
    world.insert(
        wall,
        crate::Collider::Box {
            half_extents: Vec2::ONE,
        },
    );
    world.insert(wall, crate::ShadowCaster2D::default());
    assert!(gather(&world, VIEW.0, VIEW.1, None, None)
        .occluders
        .is_empty());

    world
        .get_mut::<PointLight2D>(lamp)
        .expect("spawned with one")
        .shadows = Some(LightShadows::default());
    let lighting = gather(&world, VIEW.0, VIEW.1, None, None);
    assert_eq!(lighting.occluders.len(), 1);
    let shadow = lighting.lights[0].shadow.expect("casts");
    assert_eq!(shadow.strength, 1.0);
}

#[test]
fn shadows_are_left_out_of_the_file_when_off() {
    let text = ron::to_string(&PointLight2D::default()).expect("writes");
    assert!(!text.contains("shadows"), "{text}");
    let lit: PointLight2D = ron::from_str("(shadows: Some((strength: 0.5)))").expect("reads");
    assert_eq!(
        lit.shadows,
        Some(LightShadows {
            strength: 0.5,
            ..LightShadows::default()
        })
    );
}

#[test]
fn the_shadow_map_s_resolution_is_the_camera_s_and_left_out_when_default() {
    let mut world = World::new();
    let camera = world.spawn();
    world.insert(
        camera,
        Camera {
            ambient: Some([0.1; 3]),
            shadow_resolution: 256,
            ..Camera::default()
        },
    );
    assert_eq!(
        gather(&world, VIEW.0, VIEW.1, None, None).shadow_resolution,
        256
    );
    let text = ron::to_string(&Camera::default()).expect("writes");
    assert!(!text.contains("shadow_resolution"), "{text}");
    let read: Camera = ron::from_str(&text).expect("reads");
    assert_eq!(
        read.shadow_resolution,
        voltra_render::lights::DEFAULT_SHADOW_RESOLUTION
    );
}

#[test]
fn a_spot_light_points_along_its_entity_and_is_left_out_of_the_file_when_off() {
    let mut world = World::new();
    lit_camera(&mut world, Some([0.1; 3]));
    let spot = lamp(&mut world, Vec2::ZERO);
    if let Some(transform) = world.get_mut::<Transform>(spot) {
        transform.rotation = std::f32::consts::FRAC_PI_2;
    }
    if let Some(light) = world.get_mut::<PointLight2D>(spot) {
        light.cone = Some(LightCone::default());
    }
    let cone = gather(&world, VIEW.0, VIEW.1, None, None).lights[0]
        .cone
        .expect("a spot light");
    assert!(
        (cone.direction - Vec2::Y).length() < 1e-5,
        "turned a quarter: {cone:?}"
    );
    assert_eq!(cone.angle, LightCone::default().angle);

    let text = ron::to_string(&PointLight2D::default()).expect("writes");
    assert!(!text.contains("cone"), "{text}");
    let read: PointLight2D = ron::from_str("(cone: Some((angle: 1.0)))").expect("reads");
    assert_eq!(read.cone.map(|c| c.softness), Some(0.25));
}

#[test]
fn a_light_s_height_scales_with_its_entity_and_is_left_out_when_absent() {
    let light = PointLight2D {
        height: Some(0.5),
        ..PointLight2D::default()
    };
    let placed = light.placed(Mat3::from_scale(Vec2::splat(4.0)));
    assert_eq!(placed.height, Some(2.0));
    let text = ron::to_string(&PointLight2D::default()).expect("writes");
    assert!(!text.contains("height"), "{text}");
}

#[test]
fn a_light_s_layers_reach_the_renderer_and_all_of_them_are_left_out_of_the_file() {
    let light = PointLight2D {
        layers: 0b10,
        ..PointLight2D::default()
    };
    assert_eq!(light.placed(Mat3::IDENTITY).layers, 0b10);
    let text = ron::to_string(&PointLight2D::default()).expect("writes");
    assert!(!text.contains("layers"), "{text}");
    let read: PointLight2D = ron::from_str("(layers: 4)").expect("reads");
    assert_eq!(read.layers, 4);
}

#[test]
fn global_lights_are_gathered_only_into_a_lit_frame() {
    let mut world = World::new();
    lit_camera(&mut world, None);
    let glow = world.spawn();
    world.insert(glow, crate::GlobalLight2D::default());
    assert!(gather(&world, VIEW.0, VIEW.1, None, None)
        .globals
        .is_empty());

    let mut world = World::new();
    lit_camera(&mut world, Some([0.1; 3]));
    let glow = world.spawn();
    world.insert(glow, crate::GlobalLight2D::default());
    let globals = gather(&world, VIEW.0, VIEW.1, None, None).globals;
    assert_eq!(globals, [crate::GlobalLight2D::default().light()]);
}

#[test]
fn a_view_turned_45_degrees_gathers_by_the_box_around_its_turned_rect() {
    let mut world = World::new();
    lit_camera(&mut world, Some([0.1; 3]));
    // Ten by two, turned 45°: its box reaches about 4.24 each way.
    let view = voltra_render::Camera2D::new(Vec2::ZERO, 1.0, 5.0)
        .with_rotation(std::f32::consts::FRAC_PI_4);
    let (min, max) = view.visible_bounds();
    // Up the turned view's long axis, well above where it would reach
    // upright (radius 3 from y = 7 reaches down to 4 > 1).
    lamp(&mut world, Vec2::new(4.0, 7.0));
    // Out past the box's corner by more than its radius.
    lamp(&mut world, Vec2::new(-9.0, 9.0));
    let at: Vec<Vec2> = gather(&world, min, max, None, None)
        .lights
        .iter()
        .map(|l| l.position)
        .collect();
    assert_eq!(at, vec![Vec2::new(4.0, 7.0)]);
}
