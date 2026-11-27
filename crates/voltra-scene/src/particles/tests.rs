use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::*;
use crate::sprite::sheets::Sheets;
use crate::{SpriteBatch, Transform2D};

const FRAME: f32 = 1.0 / 60.0;

fn emitter_at(world: &mut World, at: Vec2, emitter: ParticleEmitter) -> Entity {
    let entity = world.spawn();
    world.insert(entity, Transform2D::from_translation(at));
    world.insert(entity, emitter);
    entity
}

fn get(world: &World, entity: Entity) -> &ParticleEmitter {
    world.get::<ParticleEmitter>(entity).expect("an emitter")
}

fn run(world: &mut World, seconds: f32) -> Vec<Entity> {
    let mut finished = Vec::new();
    for _ in 0..(seconds / FRAME).round() as usize {
        finished.extend(advance(world, FRAME, false));
    }
    finished
}

#[test]
fn a_rate_emits_that_many_per_second() {
    let mut world = World::new();
    let e = emitter_at(
        &mut world,
        Vec2::ZERO,
        ParticleEmitter {
            rate: 30.0,
            lifetime: (10.0, 10.0),
            ..ParticleEmitter::default()
        },
    );
    run(&mut world, 1.0);
    let alive = get(&world, e).alive();
    assert!((29..=31).contains(&alive), "{alive}");
}

#[test]
fn particles_die_at_the_end_of_their_life() {
    let mut world = World::new();
    let e = emitter_at(
        &mut world,
        Vec2::ZERO,
        ParticleEmitter {
            emitting: false,
            lifetime: (0.5, 0.5),
            ..ParticleEmitter::default()
        },
    );
    world
        .get_mut::<ParticleEmitter>(e)
        .expect("emitter")
        .burst(10);
    run(&mut world, 0.25);
    assert_eq!(get(&world, e).alive(), 10);
    run(&mut world, 0.5);
    assert_eq!(get(&world, e).alive(), 0);
}

#[test]
fn the_cap_holds_however_much_is_asked() {
    let mut world = World::new();
    let e = emitter_at(
        &mut world,
        Vec2::ZERO,
        ParticleEmitter {
            rate: 10_000.0,
            max_particles: 50,
            lifetime: (10.0, 10.0),
            ..ParticleEmitter::default()
        },
    );
    world
        .get_mut::<ParticleEmitter>(e)
        .expect("emitter")
        .burst(1_000_000);
    run(&mut world, 0.5);
    assert_eq!(get(&world, e).alive(), 50);
}

#[test]
fn world_space_particles_stay_behind_a_moving_emitter() {
    let mut world = World::new();
    let still = ParticleEmitter {
        emitting: false,
        speed: (0.0, 0.0),
        acceleration: Vec2::ZERO,
        lifetime: (5.0, 5.0),
        ..ParticleEmitter::default()
    };
    let e = emitter_at(&mut world, Vec2::new(3.0, 0.0), still);
    world
        .get_mut::<ParticleEmitter>(e)
        .expect("emitter")
        .burst(1);
    advance(&mut world, FRAME, false);
    world
        .get_mut::<Transform2D>(e)
        .expect("transform")
        .translation = Vec2::new(10.0, 0.0);
    advance(&mut world, FRAME, false);
    assert_eq!(get(&world, e).particles()[0].position, Vec2::new(3.0, 0.0));
}

#[test]
fn particles_leave_in_the_direction_within_the_spread() {
    let mut world = World::new();
    let e = emitter_at(
        &mut world,
        Vec2::ZERO,
        ParticleEmitter {
            emitting: false,
            direction: 0.0,
            spread: 0.2,
            speed: (1.0, 1.0),
            acceleration: Vec2::ZERO,
            ..ParticleEmitter::default()
        },
    );
    world
        .get_mut::<ParticleEmitter>(e)
        .expect("emitter")
        .burst(100);
    advance(&mut world, FRAME, false);
    for particle in get(&world, e).particles() {
        let angle = particle.velocity.to_angle();
        assert!(angle.abs() <= 0.1 + 1e-5, "{angle}");
    }
}

#[test]
fn acceleration_bends_the_path() {
    let mut world = World::new();
    let e = emitter_at(
        &mut world,
        Vec2::ZERO,
        ParticleEmitter {
            emitting: false,
            speed: (0.0, 0.0),
            acceleration: Vec2::new(0.0, -10.0),
            lifetime: (5.0, 5.0),
            ..ParticleEmitter::default()
        },
    );
    world
        .get_mut::<ParticleEmitter>(e)
        .expect("emitter")
        .burst(1);
    run(&mut world, 1.0);
    assert!(get(&world, e).particles()[0].position.y < -4.0);
}

#[test]
fn a_one_shot_is_reported_finished_once_its_burst_is_gone() {
    let mut world = World::new();
    let e = emitter_at(
        &mut world,
        Vec2::ZERO,
        ParticleEmitter {
            emitting: false,
            one_shot: true,
            lifetime: (0.2, 0.2),
            ..ParticleEmitter::default()
        },
    );
    world
        .get_mut::<ParticleEmitter>(e)
        .expect("emitter")
        .burst(5);
    let early = advance(&mut world, FRAME, false);
    assert!(early.is_empty(), "still sparkling");
    let finished = run(&mut world, 0.5);
    assert_eq!(finished.first(), Some(&e));
}

#[test]
fn a_stalled_frame_does_not_dump_a_second_of_particles() {
    let mut world = World::new();
    let e = emitter_at(
        &mut world,
        Vec2::ZERO,
        ParticleEmitter {
            rate: 100.0,
            lifetime: (10.0, 10.0),
            ..ParticleEmitter::default()
        },
    );
    advance(&mut world, 5.0, false);
    assert!(get(&world, e).alive() <= 10, "{}", get(&world, e).alive());
}

#[test]
fn nonsense_numbers_emit_nothing_rather_than_panic() {
    let mut world = World::new();
    let e = emitter_at(
        &mut world,
        Vec2::ZERO,
        ParticleEmitter {
            rate: f32::NAN,
            speed: (f32::INFINITY, f32::INFINITY),
            ..ParticleEmitter::default()
        },
    );
    world
        .get_mut::<ParticleEmitter>(e)
        .expect("emitter")
        .burst(3);
    run(&mut world, 0.2);
    assert_eq!(get(&world, e).alive(), 0);
    assert!(advance(&mut world, f32::NAN, false).is_empty());
}

#[test]
fn particles_are_drawn_shrinking_and_fading() {
    let mut world = World::new();
    let e = emitter_at(
        &mut world,
        Vec2::ZERO,
        ParticleEmitter {
            emitting: false,
            lifetime: (1.0, 1.0),
            speed: (0.0, 0.0),
            acceleration: Vec2::ZERO,
            size: (1.0, 0.0),
            ..ParticleEmitter::default()
        },
    );
    world
        .get_mut::<ParticleEmitter>(e)
        .expect("emitter")
        .burst(1);
    run(&mut world, 0.5);

    let batch = SpriteBatch::from_world(&world, Sheets::default());
    assert_eq!(batch.sprite_count(), 1);
    let xs: Vec<f32> = batch
        .quad_vertices()
        .iter()
        .map(|v| v.position[0])
        .collect();
    let width =
        xs.iter().copied().fold(f32::MIN, f32::max) - xs.iter().copied().fold(f32::MAX, f32::min);
    assert!(
        (width - 0.5).abs() < 0.05,
        "half way through, half the size: {width}"
    );
    let alpha = batch.quad_vertices()[0].color[3];
    assert!((alpha - 0.5).abs() < 0.05, "and half faded: {alpha}");
}

#[test]
fn an_emitter_round_trips_through_ron_without_its_particles() {
    let mut emitter = ParticleEmitter::default();
    emitter.burst(4);
    let text = ron::to_string(&emitter).expect("serialises");
    let back: ParticleEmitter = ron::from_str(&text).expect("reads");
    assert_eq!(back.rate, emitter.rate);
    assert_eq!(back.alive(), 0);
    assert!(!text.contains("state"));
}

#[test]
fn particles_are_drawn_on_their_emitter_s_light_layers() {
    let mut world = World::new();
    let e = emitter_at(
        &mut world,
        Vec2::ZERO,
        ParticleEmitter {
            emitting: false,
            light_layers: 0b1000,
            ..ParticleEmitter::default()
        },
    );
    world
        .get_mut::<ParticleEmitter>(e)
        .expect("emitter")
        .burst(3);
    run(&mut world, 0.1);
    let batch = SpriteBatch::from_world(&world, Sheets::default());
    assert!(!batch.ranges.is_empty());
    assert!(batch.ranges.iter().all(|r| r.light_layers == 0b1000));

    let plain = ron::to_string(&ParticleEmitter::default()).expect("writes");
    assert!(!plain.contains("light_layers"), "{plain}");
}
