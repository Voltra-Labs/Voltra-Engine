//! Dodge: steer through falling rocks for as long as you can — timers,
//! tweens and sensor events in a game of its own.
//!
//! A repeating timer drops a rock from a random place along the top, a
//! little more often each time; rocks are kinematic sensors falling at
//! their own speed. The first one the player begins touching ends the run:
//! the player fades out on a tween and the score — seconds survived — stops.

#![warn(missing_docs)]

use serde::{Deserialize, Serialize};
use voltra_core::{Game, Tick, Touch};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::keyframes::{Ease, Property, Value};
use voltra_scene::{
    Collider, ComponentRegistry, RigidBody, Sensor, Sprite, Text, TimerId, Transform,
};

/// The first scene, under the game's asset root.
pub const FIRST_SCENE: &str = "scenes/main.ron";

/// What the move actions steer, at `speed`, inside `arena` (half its width
/// and height, from the middle).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Player {
    /// World units a second.
    pub speed: f32,
    /// Half the box the player keeps inside, from the middle.
    pub arena: Vec2,
}

impl Player {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "Player";
}

impl Default for Player {
    fn default() -> Self {
        Self {
            speed: 6.0,
            arena: Vec2::new(8.0, 4.5),
        }
    }
}

/// A falling rock.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Rock;

impl Rock {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "Rock";
}

/// How the rocks come: one every `every` seconds at first, a little more
/// often each time down to `fastest`, falling between `speeds`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rockfall {
    /// Seconds between the first rocks.
    pub every: f32,
    /// The shortest gap the rocks quicken to.
    pub fastest: f32,
    /// The slowest and fastest fall, world units a second.
    pub speeds: Vec2,
    /// The height they start at.
    pub top: f32,
    /// Half the width they start across, from the middle.
    pub width: f32,
}

impl Rockfall {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "Rockfall";
}

impl Default for Rockfall {
    fn default() -> Self {
        Self {
            every: 0.7,
            fastest: 0.2,
            speeds: Vec2::new(3.0, 7.0),
            top: 6.0,
            width: 8.0,
        }
    }
}

/// Seconds survived, and whether the run is over. On a component, so it
/// lives through the editor reloading this code.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Run {
    /// Seconds survived so far.
    pub seconds: f32,
    /// A rock has hit the player.
    pub over: bool,
}

impl Run {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "Run";
}

/// The game. Made afresh for every play in the editor.
#[derive(Debug, Default)]
pub struct Dodge {
    /// The rock timer, made on the first turn.
    timer: Option<TimerId>,
    /// Rocks dropped so far, which quickens the next.
    dropped: u32,
    /// A small random walk for where rocks fall: deterministic, so a test
    /// sees the same run every time.
    seed: u32,
}

impl Game for Dodge {
    fn register(&self, registry: &mut ComponentRegistry) {
        registry.register::<Player>(Player::NAME).with_template();
        registry.register::<Rock>(Rock::NAME).with_template();
        registry
            .register::<Rockfall>(Rockfall::NAME)
            .with_template();
        registry.register::<Run>(Run::NAME).with_template();
    }

    fn fixed_update(&mut self, tick: &mut Tick<'_>) {
        let Some((run_entity, run)) = first::<Run>(tick.world) else {
            return;
        };
        if run.over {
            return;
        }
        steer(tick);
        let fall = first::<Rockfall>(tick.world)
            .map(|(_, f)| f)
            .unwrap_or_default();
        let timer = *self
            .timer
            .get_or_insert_with(|| tick.timers.after(fall.every, None));
        if tick.timers.fired(timer) {
            self.drop_rock(tick.world, fall);
            let next = (fall.every * 0.97f32.powi(self.dropped as i32)).max(fall.fastest);
            self.timer = Some(tick.timers.after(next, None));
        }
        clear_fallen(tick.world, fall);
        let hit = hit_player(tick);
        if let Some(r) = tick.world.get_mut::<Run>(run_entity) {
            r.seconds += tick.delta;
            r.over = hit.is_some();
        }
        if let Some(player) = hit {
            // Out, fading over half a second.
            tick.tween(
                player,
                Property::SpriteColor,
                Value::Color([1.0, 0.3, 0.3, 0.0]),
                0.5,
                Ease::EaseOut,
            );
            if let Some(body) = tick.world.get_mut::<RigidBody>(player) {
                body.velocity = Vec2::ZERO;
            }
        }
    }

    fn update(&mut self, tick: &mut Tick<'_>) {
        let Some((entity, run)) = first::<Run>(tick.world) else {
            return;
        };
        let words = if run.over {
            format!("Hit! You lasted {:.1} s", run.seconds)
        } else {
            format!("{:.1} s", run.seconds)
        };
        if let Some(text) = tick.world.get_mut::<Text>(entity) {
            if text.content != words {
                text.content = words;
            }
        }
    }
}

impl Dodge {
    /// The next number of the walk, `0` to `1`.
    fn random(&mut self) -> f32 {
        // xorshift32, seeded on first use.
        let mut x = if self.seed == 0 {
            0x9E37_79B9
        } else {
            self.seed
        };
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.seed = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    fn drop_rock(&mut self, world: &mut World, fall: Rockfall) {
        let x = (self.random() * 2.0 - 1.0) * fall.width;
        let speed = fall.speeds.x + self.random() * (fall.speeds.y - fall.speeds.x);
        let size = 0.5 + self.random() * 0.5;
        let rock = world.spawn();
        world.insert(rock, Rock);
        world.insert(
            rock,
            Transform::from_translation(Vec2::new(x, fall.top)).with_scale(Vec2::splat(size)),
        );
        world.insert(rock, Sprite::new([0.6, 0.5, 0.45, 1.0]).with_sort_order(1));
        world.insert(rock, Collider::Circle { radius: 0.5 });
        world.insert(rock, Sensor);
        world.insert(
            rock,
            RigidBody {
                body_type: voltra_scene::body::BodyType::Kinematic,
                velocity: Vec2::new(0.0, -speed),
                ..RigidBody::default()
            },
        );
        self.dropped += 1;
    }
}

/// The first entity with a `T`, and a copy of it.
fn first<T: Copy + 'static>(world: &World) -> Option<(Entity, T)> {
    world.query::<T>().next().map(|(e, t)| (e, *t))
}

/// The player at the move actions' velocity, held inside the arena.
fn steer(tick: &mut Tick<'_>) {
    let Some((entity, player)) = first::<Player>(tick.world) else {
        return;
    };
    let wish = Vec2::new(
        tick.input.action_axis("move_left", "move_right"),
        tick.input.action_axis("move_down", "move_up"),
    )
    .clamp_length_max(1.0);
    let at = tick
        .world
        .get::<Transform>(entity)
        .map_or(Vec2::ZERO, |t| t.translation);
    let mut velocity = wish * player.speed;
    // Not out of the arena: a kinematic body is stopped by nothing.
    for axis in 0..2 {
        let past =
            at[axis].abs() >= player.arena[axis] && at[axis].signum() == velocity[axis].signum();
        if past {
            velocity[axis] = 0.0;
        }
    }
    if let Some(body) = tick.world.get_mut::<RigidBody>(entity) {
        body.velocity = velocity;
    }
}

/// Rocks fallen out of sight go.
fn clear_fallen(world: &mut World, fall: Rockfall) {
    let gone: Vec<Entity> = world
        .query::<Rock>()
        .filter(|(e, _)| {
            world
                .get::<Transform>(*e)
                .is_some_and(|t| t.translation.y < -fall.top)
        })
        .map(|(e, _)| e)
        .collect();
    for rock in gone {
        world.despawn(rock);
    }
}

/// The player, if a rock began touching it this step.
fn hit_player(tick: &Tick<'_>) -> Option<Entity> {
    let (player, _) = first::<Player>(tick.world)?;
    tick.events
        .iter()
        .filter(|event| event.touch == Touch::Began)
        .filter_map(|event| event.other(player))
        .any(|other| tick.world.get::<Rock>(other).is_some())
        .then_some(player)
}

voltra_core::export_game!(Dodge::default());
