//! Breakout: a paddle, a ball and a wall of bricks — the physics, its
//! collision events and a HUD, in a game of its own.
//!
//! The ball is a dynamic body with no gravity and a bounce of one; the
//! game keeps its speed steady and its angle off the horizontal, tilts it
//! by where it meets the paddle, and takes each brick it begins touching.
//! Everything else — walls, rebounds, the ball not tunnelling — is the
//! engine's.

#![warn(missing_docs)]

use serde::{Deserialize, Serialize};
use voltra_core::{Game, Tick, Touch};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{ComponentRegistry, RigidBody2D, Text, Transform2D};

/// The first scene, under the game's asset root.
pub const FIRST_SCENE: &str = "scenes/main.ron";

/// Below this the ball is lost and served again.
const FLOOR: f32 = -6.0;

/// The paddle: steered by the move actions at `speed` world units a
/// second, kept within `reach` of the middle.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Paddle {
    /// World units a second.
    pub speed: f32,
    /// How far from the middle it may go.
    pub reach: f32,
}

impl Paddle {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "Paddle";
}

impl Default for Paddle {
    fn default() -> Self {
        Self {
            speed: 10.0,
            reach: 7.0,
        }
    }
}

/// The ball: always moving at `speed`, and served from `serve` when lost.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ball {
    /// World units a second, held steady.
    pub speed: f32,
    /// Where a new ball starts.
    pub serve: Vec2,
}

impl Ball {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "Ball";
}

impl Default for Ball {
    fn default() -> Self {
        Self {
            speed: 7.0,
            serve: Vec2::new(0.0, -3.0),
        }
    }
}

/// A brick, worth `points` when the ball breaks it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Brick {
    /// Added to the score when it breaks.
    pub points: u32,
}

impl Brick {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "Brick";
}

impl Default for Brick {
    fn default() -> Self {
        Self { points: 10 }
    }
}

/// The score and the balls left, shown in its entity's text. On a
/// component, so it lives through the editor reloading this code.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Score {
    /// Points so far.
    pub points: u32,
    /// Balls left, the one in play included.
    pub balls: u32,
}

impl Score {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "Score";
}

impl Default for Score {
    fn default() -> Self {
        Self {
            points: 0,
            balls: 3,
        }
    }
}

/// The game. Made afresh for every play in the editor.
#[derive(Debug, Default)]
pub struct Breakout;

impl Game for Breakout {
    fn register(&self, registry: &mut ComponentRegistry) {
        registry.register::<Paddle>(Paddle::NAME).with_template();
        registry.register::<Ball>(Ball::NAME).with_template();
        registry.register::<Brick>(Brick::NAME).with_template();
        registry.register::<Score>(Score::NAME).with_template();
    }

    fn fixed_update(&mut self, tick: &mut Tick<'_>) {
        steer_paddle(tick);
        let Some((ball, settings)) = first::<Ball>(tick.world) else {
            return;
        };
        let broken = break_bricks(tick, ball);
        let tilt = paddle_tilt(tick, ball);
        keep_moving(tick.world, ball, settings, tilt);
        let lost = tick
            .world
            .get::<Transform2D>(ball)
            .is_some_and(|t| t.translation.y < FLOOR);
        if lost {
            serve(tick.world, ball, settings);
        }
        if let Some((entity, mut score)) = first::<Score>(tick.world) {
            score.points += broken;
            if lost {
                score.balls = score.balls.saturating_sub(1);
            }
            if broken > 0 || lost {
                if let Some(s) = tick.world.get_mut::<Score>(entity) {
                    *s = score;
                }
            }
        }
    }

    fn update(&mut self, tick: &mut Tick<'_>) {
        let Some((entity, score)) = first::<Score>(tick.world) else {
            return;
        };
        let left = tick.world.query::<Brick>().count();
        let words = if left == 0 {
            format!("Cleared! {} points", score.points)
        } else if score.balls == 0 {
            format!("Game over: {} points", score.points)
        } else {
            format!("Score {}   Balls {}", score.points, score.balls)
        };
        if let Some(text) = tick.world.get_mut::<Text>(entity) {
            if text.content != words {
                text.content = words;
            }
        }
    }
}

/// The first entity with a `T`, and a copy of it.
fn first<T: Copy + 'static>(world: &World) -> Option<(Entity, T)> {
    world.query::<T>().next().map(|(e, t)| (e, *t))
}

/// The paddle follows the move actions, as a kinematic body: its velocity
/// is what it pushes the ball with.
fn steer_paddle(tick: &mut Tick<'_>) {
    let Some((entity, paddle)) = first::<Paddle>(tick.world) else {
        return;
    };
    let mut push = tick.input.action_axis("move_left", "move_right");
    let x = tick
        .world
        .get::<Transform2D>(entity)
        .map_or(0.0, |t| t.translation.x);
    // Not past its reach: a kinematic body is stopped by nothing.
    if (x >= paddle.reach && push > 0.0) || (x <= -paddle.reach && push < 0.0) {
        push = 0.0;
    }
    if let Some(body) = tick.world.get_mut::<RigidBody2D>(entity) {
        body.velocity = Vec2::new(push * paddle.speed, 0.0);
    }
}

/// Takes every brick the ball began touching this step; what they were
/// worth.
fn break_bricks(tick: &mut Tick<'_>, ball: Entity) -> u32 {
    let hit: Vec<(Entity, u32)> = tick
        .events
        .iter()
        .filter(|event| event.touch == Touch::Began)
        .filter_map(|event| event.other(ball))
        .filter_map(|other| tick.world.get::<Brick>(other).map(|b| (other, b.points)))
        .collect();
    let mut worth = 0;
    for (brick, points) in hit {
        if tick.world.despawn(brick) {
            worth += points;
        }
    }
    worth
}

/// How far from the paddle's middle the ball met it this step, `-1` to
/// `1`, if it did: what it is tilted by, as in every Breakout.
fn paddle_tilt(tick: &Tick<'_>, ball: Entity) -> Option<f32> {
    let (paddle, settings) = first::<Paddle>(tick.world)?;
    let met = tick
        .events
        .iter()
        .any(|event| event.touch == Touch::Began && event.other(ball) == Some(paddle));
    if !met {
        return None;
    }
    let _ = settings;
    let at = |e| tick.world.get::<Transform2D>(e).map(|t| t.translation);
    let (ball_at, paddle_at) = (at(ball)?, at(paddle)?);
    let half = tick
        .world
        .get::<Transform2D>(paddle)
        .map_or(1.0, |t| t.scale.x * 0.5);
    Some(((ball_at.x - paddle_at.x) / half.max(0.1)).clamp(-1.0, 1.0))
}

/// The ball at its speed, never too flat to come back, tilted by the
/// paddle when it just met it.
fn keep_moving(world: &mut World, ball: Entity, settings: Ball, tilt: Option<f32>) {
    let Some(body) = world.get_mut::<RigidBody2D>(ball) else {
        return;
    };
    let mut velocity = body.velocity;
    if let Some(tilt) = tilt {
        velocity = Vec2::new(tilt * 0.8, 1.0);
    }
    if velocity.length_squared() < 1e-6 {
        velocity = Vec2::new(0.6, 1.0);
    }
    let mut direction = velocity.normalize();
    // A third of the speed upwards or downwards at least.
    if direction.y.abs() < 0.35 {
        direction.y = 0.35f32.copysign(if direction.y == 0.0 { 1.0 } else { direction.y });
        direction = direction.normalize();
    }
    body.velocity = direction * settings.speed;
}

/// The ball back above the paddle, on its way up.
fn serve(world: &mut World, ball: Entity, settings: Ball) {
    if let Some(transform) = world.get_mut::<Transform2D>(ball) {
        transform.translation = settings.serve;
    }
    if let Some(body) = world.get_mut::<RigidBody2D>(ball) {
        body.velocity = Vec2::new(0.6, 1.0).normalize() * settings.speed;
    }
}

voltra_core::export_game!(Breakout);
