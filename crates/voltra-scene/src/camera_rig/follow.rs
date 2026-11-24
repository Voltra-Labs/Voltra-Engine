//! Following a target: smoothed, with a dead zone, a lead, and limits.

use serde::{Deserialize, Serialize};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;

use super::{set_world_position, world_position};
use crate::format::{entity_with_id, SceneRefs};
use crate::scene_id::SceneId;
use crate::Camera;

/// Keeps the entity — usually a camera — on `target`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CameraFollow2D {
    /// What it follows, by identity; `None` follows nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<SceneId>,
    /// Where it sits from the target, in world units: a camera that shows
    /// more of what is above the player than below.
    pub offset: Vec2,
    /// How fast it closes on where it should be: the share of the distance
    /// left closed per second, as an exponential rate, so the frame rate
    /// does not change where it ends up. Zero snaps.
    pub smoothing: f32,
    /// Half the size of the box, around its own centre, the target moves in
    /// without moving it — Godot's drag margins, Cinemachine's dead zone —
    /// in world units.
    pub dead_zone: Vec2,
    /// Seconds of the target's velocity it looks ahead by. Zero looks at
    /// the target.
    pub look_ahead: f32,
    /// What a camera on it may show, or where any other entity may be: kept
    /// inside, its view and all. `None` is unbounded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<FollowBounds>,
    /// What it remembers from frame to frame. Never saved.
    #[serde(skip)]
    pub state: FollowState,
}

/// A follower's memory between frames.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FollowState {
    /// Where the target was last frame, for its velocity; `None` snaps on
    /// the next frame.
    last_target: Option<Vec2>,
    lead: Vec2,
    /// The target last said missing, so it is said once.
    missing: Option<SceneId>,
}

/// A rectangle in the world.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FollowBounds {
    /// The corner with the smallest x and y, in world units.
    pub min: Vec2,
    /// The corner with the largest x and y, in world units.
    pub max: Vec2,
}

impl Default for CameraFollow2D {
    fn default() -> Self {
        Self {
            target: None,
            offset: Vec2::ZERO,
            smoothing: 5.0,
            dead_zone: Vec2::ZERO,
            look_ahead: 0.0,
            bounds: None,
            state: FollowState::default(),
        }
    }
}

impl CameraFollow2D {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "CameraFollow2D";

    /// Following `target`, with the defaults.
    pub fn new(target: SceneId) -> Self {
        Self {
            target: Some(target),
            ..Self::default()
        }
    }

    /// Lands on the target next frame instead of travelling to it — after
    /// a teleport, a scene change, a new target.
    pub fn snap(&mut self) {
        self.state.last_target = None;
        self.state.lead = Vec2::ZERO;
    }
}

impl SceneRefs for CameraFollow2D {
    fn remap(&mut self, map: &dyn Fn(SceneId) -> Option<SceneId>) {
        if let Some(id) = self.target {
            self.target = Some(map(id).unwrap_or(id));
        }
    }
}

/// One frame of every follower.
pub(super) fn update(world: &mut World, delta: f32, aspect: f32) {
    let followers: Vec<Entity> = world
        .query::<CameraFollow2D>()
        .map(|(entity, _)| entity)
        .collect();
    for entity in followers {
        follow_one(world, entity, delta, aspect);
    }
}

fn follow_one(world: &mut World, entity: Entity, delta: f32, aspect: f32) {
    let Some(rig) = world.get::<CameraFollow2D>(entity).cloned() else {
        return;
    };
    let Some(id) = rig.target else {
        return;
    };
    let Some(target) = entity_with_id(world, id).filter(|&t| t != entity) else {
        if rig.state.missing != Some(id) {
            log::warn!("a follower's target {id:?} is not in the scene; it holds still");
            if let Some(rig) = world.get_mut::<CameraFollow2D>(entity) {
                rig.state.missing = Some(id);
            }
        }
        return;
    };
    let at = world_position(world, target);
    let here = world_position(world, entity);
    let half_view = world
        .get::<Camera>(entity)
        .map_or(Vec2::ZERO, |c| Vec2::new(c.size * aspect.max(0.0), c.size));

    let first = rig.state.last_target.is_none();
    let rate = if first || rig.smoothing <= 0.0 || !rig.smoothing.is_finite() {
        1.0
    } else {
        1.0 - (-rig.smoothing * delta).exp()
    };
    let velocity = match rig.state.last_target {
        Some(was) if delta > 0.0 => (at - was) / delta,
        _ => Vec2::ZERO,
    };
    let wanted_lead = velocity * rig.look_ahead.max(0.0);
    let lead = if first {
        Vec2::ZERO
    } else {
        rig.state.lead + (wanted_lead - rig.state.lead) * rate
    };

    let aim = at + rig.offset + lead;
    let goal = clamp_to(
        dead_zone(here, aim, rig.dead_zone.max(Vec2::ZERO)),
        rig.bounds,
        half_view,
    );
    let next = here + (goal - here) * rate;
    if next.is_finite() {
        set_world_position(world, entity, next);
    }
    if let Some(rig) = world.get_mut::<CameraFollow2D>(entity) {
        rig.state.last_target = Some(at);
        rig.state.lead = if lead.is_finite() { lead } else { Vec2::ZERO };
        rig.state.missing = None;
    }
}

/// Where the centre should be for `aim` to sit inside a box of `half`
/// around it, moving as little as possible from `here`.
fn dead_zone(here: Vec2, aim: Vec2, half: Vec2) -> Vec2 {
    let offset = aim - here;
    here + offset - offset.clamp(-half, half)
}

/// `centre` moved so a view of `half` around it stays inside `bounds`; a
/// view wider than the bounds is centred on them.
fn clamp_to(centre: Vec2, bounds: Option<FollowBounds>, half: Vec2) -> Vec2 {
    let Some(FollowBounds { min, max }) = bounds else {
        return centre;
    };
    let (lo, hi) = (min.min(max), min.max(max));
    let mut out = centre;
    for axis in 0..2 {
        let (from, to) = (lo[axis] + half[axis], hi[axis] - half[axis]);
        out[axis] = if from > to {
            (lo[axis] + hi[axis]) * 0.5
        } else {
            centre[axis].clamp(from, to)
        };
    }
    out
}
