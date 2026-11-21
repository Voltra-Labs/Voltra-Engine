//! What the CPU plans for a GPU emitter: births counted as on the CPU,
//! slots handed out round the ring and never while their particle may live,
//! and the emitter's place in the batch's draw order. No device needed.

use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::*;
use crate::sprite::sheets::Sheets;
use crate::{Sprite, SpriteBatch, Transform};

const FRAME: f32 = 1.0 / 60.0;

fn gpu_emitter(world: &mut World, emitter: ParticleEmitter) -> Entity {
    let entity = world.spawn();
    world.insert(entity, Transform::from_translation(Vec2::new(3.0, 4.0)));
    world.insert(
        entity,
        ParticleEmitter {
            simulation: Simulation::Gpu,
            ..emitter
        },
    );
    entity
}

fn emitter(world: &mut World, entity: Entity) -> &mut ParticleEmitter {
    world
        .get_mut::<ParticleEmitter>(entity)
        .expect("an emitter")
}

#[test]
fn births_go_to_the_ring_not_the_cpu() {
    let mut world = World::new();
    let e = gpu_emitter(
        &mut world,
        ParticleEmitter {
            emitting: false,
            max_particles: 64,
            ..ParticleEmitter::default()
        },
    );
    emitter(&mut world, e).burst(5);
    advance(&mut world, FRAME, true);

    let em = emitter(&mut world, e);
    assert!(em.particles().is_empty(), "nothing on the CPU");
    assert_eq!(em.gpu_capacity(), Some(64));
    assert_eq!(em.alive(), 5);
    let step = em.take_gpu_step().expect("a step is owed");
    assert_eq!((step.spawn_start, step.spawn_count), (0, 5));
    assert_eq!(
        step.origin,
        Vec2::new(3.0, 4.0),
        "world space: born where it is"
    );
    assert!(em.take_gpu_step().is_none(), "taken once");
}

#[test]
fn steps_between_two_frames_are_one_dispatch() {
    let mut world = World::new();
    let e = gpu_emitter(
        &mut world,
        ParticleEmitter {
            emitting: false,
            ..ParticleEmitter::default()
        },
    );
    emitter(&mut world, e).burst(2);
    advance(&mut world, FRAME, true);
    emitter(&mut world, e).burst(3);
    advance(&mut world, FRAME, true);
    advance(&mut world, FRAME, true);

    let step = emitter(&mut world, e).take_gpu_step().expect("owed");
    assert_eq!((step.spawn_start, step.spawn_count), (0, 5));
    assert!((step.dt - 3.0 * FRAME).abs() < 1e-6);
}

#[test]
fn births_wait_for_room_and_slots_wrap() {
    let mut world = World::new();
    let e = gpu_emitter(
        &mut world,
        ParticleEmitter {
            emitting: false,
            max_particles: 4,
            lifetime: (0.5, 0.5),
            ..ParticleEmitter::default()
        },
    );
    emitter(&mut world, e).burst(10);
    advance(&mut world, FRAME, true);
    let first = emitter(&mut world, e).take_gpu_step().expect("owed");
    assert_eq!(first.spawn_count, 4, "the ring is full");

    // Still alive as far as the ledger knows: no slot handed out.
    emitter(&mut world, e).burst(2);
    advance(&mut world, 0.3, true);
    assert_eq!(
        emitter(&mut world, e)
            .take_gpu_step()
            .map(|s| s.spawn_count),
        Some(0)
    );

    // Past the longest lifetime and the margin: the ring is free again, and
    // the next births start where the last left off.
    for _ in 0..6 {
        advance(&mut world, 0.1, true);
    }
    emitter(&mut world, e).take_gpu_step();
    emitter(&mut world, e).burst(2);
    advance(&mut world, FRAME, true);
    let later = emitter(&mut world, e).take_gpu_step().expect("owed");
    assert_eq!(
        (later.spawn_start, later.spawn_count),
        (0, 2),
        "4 % 4 wraps to 0"
    );
}

#[test]
fn without_a_gpu_it_runs_on_the_cpu() {
    let mut world = World::new();
    let e = gpu_emitter(
        &mut world,
        ParticleEmitter {
            emitting: false,
            ..ParticleEmitter::default()
        },
    );
    emitter(&mut world, e).burst(3);
    advance(&mut world, FRAME, false);
    let em = emitter(&mut world, e);
    assert_eq!(em.particles().len(), 3);
    assert_eq!(em.gpu_capacity(), None);
}

#[test]
fn switched_back_to_the_cpu_the_ring_goes() {
    let mut world = World::new();
    let e = gpu_emitter(&mut world, ParticleEmitter::default());
    advance(&mut world, FRAME, true);
    assert!(emitter(&mut world, e).gpu_capacity().is_some());
    emitter(&mut world, e).simulation = Simulation::Cpu;
    advance(&mut world, FRAME, true);
    assert_eq!(emitter(&mut world, e).gpu_capacity(), None);
}

#[test]
fn a_one_shot_gpu_emitter_finishes_once_its_particles_are_dead() {
    let mut world = World::new();
    let e = gpu_emitter(
        &mut world,
        ParticleEmitter {
            emitting: false,
            one_shot: true,
            lifetime: (0.2, 0.2),
            ..ParticleEmitter::default()
        },
    );
    emitter(&mut world, e).burst(3);
    // Reported every advance once finished, for the caller to despawn; the
    // first is past the lifetime, not before.
    let first = (1..60)
        .find(|_| !advance(&mut world, FRAME, true).is_empty())
        .expect("finishes");
    let seconds = first as f32 * FRAME;
    assert!((0.2..0.4).contains(&seconds), "finished after {seconds} s");
}

#[test]
fn the_batch_draws_a_gpu_emitter_between_the_sprites_around_it() {
    let mut world = World::new();
    let sprite = |world: &mut World, sort_order| {
        let entity = world.spawn();
        world.insert(entity, Transform::default());
        world.insert(
            entity,
            Sprite {
                sort_order,
                ..Sprite::default()
            },
        );
    };
    sprite(&mut world, -1);
    sprite(&mut world, 1);
    let e = gpu_emitter(&mut world, ParticleEmitter::default());
    emitter(&mut world, e).burst(4);
    advance(&mut world, FRAME, true);

    let batch = SpriteBatch::from_world(&world, Sheets::default());
    assert_eq!(
        batch.ranges.len(),
        2,
        "split around the emitter, same texture or not"
    );
    assert_eq!(batch.gpu_emitters.len(), 1);
    assert_eq!(batch.gpu_emitters[0].entity, e);
    assert_eq!(batch.gpu_emitters[0].before, 1);
}

#[test]
fn a_gpu_setting_is_written_and_the_cpu_one_is_not() {
    let cpu = ron::to_string(&ParticleEmitter::default()).expect("writes");
    assert!(!cpu.contains("simulation"));
    let gpu = ParticleEmitter {
        simulation: Simulation::Gpu,
        ..ParticleEmitter::default()
    };
    let text = ron::to_string(&gpu).expect("writes");
    let back: ParticleEmitter = ron::from_str(&text).expect("reads");
    assert_eq!(back.simulation, Simulation::Gpu);
}

#[test]
fn an_emitter_with_nothing_alive_or_nothing_in_view_is_not_drawn() {
    let mut world = World::new();
    let e = gpu_emitter(
        &mut world,
        ParticleEmitter {
            emitting: false,
            speed: (1.0, 1.0),
            lifetime: (1.0, 1.0),
            acceleration: Vec2::ZERO,
            size: (0.2, 0.2),
            ..ParticleEmitter::default()
        },
    );
    advance(&mut world, FRAME, true);
    let all = Sheets::default();
    assert!(
        SpriteBatch::from_world(&world, all).gpu_emitters.is_empty(),
        "none born yet"
    );

    emitter(&mut world, e).burst(8);
    advance(&mut world, FRAME, true);
    assert_eq!(SpriteBatch::from_world(&world, all).gpu_emitters.len(), 1);

    // Born at (3, 4), reaching 1.1 at most: a view beside it but clear of
    // that reach does not draw it, one overlapping the reach does.
    let clear = all.visible_in(Vec2::new(5.0, 3.0), Vec2::new(8.0, 5.0));
    assert!(SpriteBatch::from_world(&world, clear)
        .gpu_emitters
        .is_empty());
    let edge = all.visible_in(Vec2::new(4.0, 3.0), Vec2::new(8.0, 5.0));
    assert_eq!(SpriteBatch::from_world(&world, edge).gpu_emitters.len(), 1);

    // Long dead: nothing to draw anywhere.
    for _ in 0..80 {
        advance(&mut world, FRAME, true);
    }
    assert!(SpriteBatch::from_world(&world, all).gpu_emitters.is_empty());
}

#[test]
fn world_space_bounds_stay_where_the_particles_were_born() {
    let mut world = World::new();
    let e = gpu_emitter(
        &mut world,
        ParticleEmitter {
            emitting: false,
            speed: (0.0, 0.0),
            acceleration: Vec2::ZERO,
            size: (1.0, 1.0),
            lifetime: (5.0, 5.0),
            ..ParticleEmitter::default()
        },
    );
    emitter(&mut world, e).burst(1);
    advance(&mut world, FRAME, true);
    // The emitter flies off; its sparks stay behind at (3, 4).
    world.insert(e, Transform::from_translation(Vec2::new(100.0, 0.0)));
    advance(&mut world, FRAME, true);
    let matrix = Transform::from_translation(Vec2::new(100.0, 0.0)).matrix();
    let (min, max) = emitter(&mut world, e).gpu_bounds(matrix).expect("alive");
    assert!(min.x <= 3.0 && max.x >= 3.0 && max.x < 10.0, "{min} {max}");
}
