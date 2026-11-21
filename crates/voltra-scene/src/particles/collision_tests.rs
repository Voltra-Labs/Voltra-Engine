//! Emitters with collision, advanced through the world: CPU particles land
//! on the scene's colliders, the mask and sensors decide which, and a GPU
//! emitter is handed the shapes its particles can reach. No device needed.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_render::ParticleCollider;

use super::*;
use crate::{Collider, CollisionLayers, Sensor, Transform};

const FRAME: f32 = 1.0 / 60.0;

/// A wide floor whose top is at y = -1.
fn floor(world: &mut World) -> Entity {
    let floor = world.spawn();
    world.insert(floor, Transform::from_translation(Vec2::new(0.0, -2.0)));
    world.insert(
        floor,
        Collider::Box {
            half_extents: Vec2::new(50.0, 1.0),
        },
    );
    floor
}

/// Rain: one burst falling straight down from the origin, living long
/// enough to land and rest.
fn rain(world: &mut World, collision: Option<ParticleCollision>, simulation: Simulation) -> Entity {
    let entity = world.spawn();
    world.insert(entity, Transform::default());
    let mut emitter = ParticleEmitter {
        emitting: false,
        speed: (4.0, 4.0),
        direction: -std::f32::consts::FRAC_PI_2,
        spread: 0.0,
        acceleration: Vec2::new(0.0, -10.0),
        lifetime: (5.0, 5.0),
        simulation,
        collision,
        ..ParticleEmitter::default()
    };
    emitter.burst(8);
    world.insert(entity, emitter);
    entity
}

fn landing() -> Option<ParticleCollision> {
    Some(ParticleCollision {
        bounce: 0.0,
        friction: 0.0,
        ..ParticleCollision::default()
    })
}

fn lowest(world: &World, entity: Entity) -> f32 {
    world
        .get::<ParticleEmitter>(entity)
        .expect("an emitter")
        .particles()
        .iter()
        .map(|p| p.position.y)
        .fold(f32::INFINITY, f32::min)
}

fn run(world: &mut World, frames: usize) {
    for _ in 0..frames {
        advance(world, FRAME, true);
    }
}

#[test]
fn cpu_particles_land_on_a_collider_and_stay_on_it() {
    let mut world = World::new();
    floor(&mut world);
    let e = rain(&mut world, landing(), Simulation::Cpu);
    run(&mut world, 120);
    let low = lowest(&world, e);
    assert!(low >= -1.0, "none fell through: {low}");
    assert!(low < -0.99, "they reached it: {low}");
    assert_eq!(world.get::<ParticleEmitter>(e).map(|e| e.alive()), Some(8));
}

#[test]
fn without_collision_they_fall_through() {
    let mut world = World::new();
    floor(&mut world);
    let e = rain(&mut world, None, Simulation::Cpu);
    run(&mut world, 120);
    assert!(lowest(&world, e) < -5.0);
}

#[test]
fn the_mask_picks_which_layers_are_hit() {
    let mut world = World::new();
    let f = floor(&mut world);
    world.insert(f, CollisionLayers::on(3));
    let e = rain(
        &mut world,
        Some(ParticleCollision {
            mask: CollisionLayers::bit(2),
            ..landing().expect("set")
        }),
        Simulation::Cpu,
    );
    run(&mut world, 120);
    assert!(lowest(&world, e) < -5.0, "layer 3 is not in the mask");
}

#[test]
fn a_sensor_is_not_solid_to_particles() {
    let mut world = World::new();
    let f = floor(&mut world);
    world.insert(f, Sensor);
    let e = rain(&mut world, landing(), Simulation::Cpu);
    run(&mut world, 120);
    assert!(lowest(&world, e) < -5.0);
}

#[test]
fn a_full_lifetime_loss_kills_what_lands() {
    let mut world = World::new();
    floor(&mut world);
    let e = rain(
        &mut world,
        Some(ParticleCollision {
            lifetime_loss: 1.0,
            ..ParticleCollision::default()
        }),
        Simulation::Cpu,
    );
    run(&mut world, 120);
    assert_eq!(world.get::<ParticleEmitter>(e).map(|e| e.alive()), Some(0));
}

#[test]
fn a_gpu_emitter_is_handed_only_the_shapes_it_can_reach() {
    let mut world = World::new();
    floor(&mut world);
    // Far past anything a particle living five seconds at these speeds
    // can reach.
    let far = world.spawn();
    world.insert(far, Transform::from_translation(Vec2::new(1000.0, 0.0)));
    world.insert(far, Collider::Circle { radius: 1.0 });
    let e = rain(&mut world, landing(), Simulation::Gpu);
    run(&mut world, 1);
    let colliders = world
        .get::<ParticleEmitter>(e)
        .expect("an emitter")
        .gpu_colliders()
        .to_vec();
    assert_eq!(
        colliders,
        [ParticleCollider::Box {
            center: Vec2::new(0.0, -2.0),
            rotation: 0.0,
            half_extents: Vec2::new(50.0, 1.0),
        }]
    );
    let step = world
        .get_mut::<ParticleEmitter>(e)
        .and_then(ParticleEmitter::take_gpu_step)
        .expect("a step");
    assert_eq!(step.response.bounce, 0.0);
}

#[test]
fn a_gpu_emitter_without_collision_is_handed_nothing() {
    let mut world = World::new();
    floor(&mut world);
    let e = rain(&mut world, None, Simulation::Gpu);
    run(&mut world, 1);
    assert!(world
        .get::<ParticleEmitter>(e)
        .is_some_and(|e| e.gpu_colliders().is_empty()));
}

#[test]
fn collision_round_trips_through_ron_and_is_left_out_when_off() {
    let emitter = ParticleEmitter {
        collision: Some(ParticleCollision {
            bounce: 0.25,
            ..ParticleCollision::default()
        }),
        ..ParticleEmitter::default()
    };
    let text = ron::to_string(&emitter).expect("serialise");
    let back: ParticleEmitter = ron::from_str(&text).expect("deserialise");
    assert_eq!(back.collision, emitter.collision);
    let plain = ron::to_string(&ParticleEmitter::default()).expect("serialise");
    assert!(!plain.contains("collision"), "{plain}");
}
