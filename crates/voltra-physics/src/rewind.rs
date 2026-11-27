//! The world's colliders as they were: a ring of past poses, and queries
//! run against them — lag compensation's half that knows no network.
//!
//! A server remembers, once per tick, where each [`LagCompensated`]
//! entity's collider was ([`PoseHistory::record`]). Given a moment of the
//! past in ticks — fractional, the moment a client was drawing when it
//! fired — [`PoseHistory::rewound`] answers a [`Rewound`]: the same ray,
//! shape, point and box queries [`crate::query`] runs, with each
//! lag-compensated collider placed between the two recorded ticks around
//! that moment (interpolated as a client draws it,
//! [`Interpolate`](voltra_scene::interpolate::Interpolate)). Everything
//! else — walls, entities not marked — is queried as it is now: a wall
//! has not moved, and what was not recorded cannot be put back.
//!
//! The live world is never changed. The rewound poses are a table the
//! queries read in place of the world's, so nothing else — another
//! system, a panic halfway — can see a collider where it no longer is.
//! Unity Netcode's `PhysicsWorldHistory` keeps copies of whole collision
//! worlds the same way; Source moves its players back and forth
//! (`StartLagCompensation` / `FinishLagCompensation`), which needs every
//! reader of a position to know a rewind is in progress.
//!
//! Which entities the past holds:
//!
//! - one **spawned after** the moment (not in the tick at or before it) is
//!   not there: nothing to hit;
//! - one **despawned since** is not either — it is not in the world to be
//!   reported, as in Source, where a player gone is gone from every
//!   rewind. A shot at a body that has just been removed misses;
//! - one recorded at the tick before the moment but not the tick after
//!   (it lost its collider in between) is held at the earlier pose.
//!
//! The collider's *shape* is recorded with its pose, so a crouch that
//! swaps a tall box for a short one is tested at the shape it had.

use std::collections::VecDeque;
use std::time::Duration;

use voltra_ecs::{Entity, World};
use voltra_scene::interpolate::Interpolate;
use voltra_scene::{Collider, LagCompensated, Transform};

use crate::query::Poses;

mod rewound;

pub use rewound::Rewound;

/// One tick's poses, sorted by entity.
#[derive(Debug, Clone)]
struct Entry {
    tick: u32,
    poses: Vec<(Entity, Collider, Transform)>,
}

impl Entry {
    fn find(&self, entity: Entity) -> Option<&(Entity, Collider, Transform)> {
        self.poses
            .binary_search_by_key(&entity, |(e, _, _)| *e)
            .ok()
            .map(|i| &self.poses[i])
    }
}

/// The last few ticks of every [`LagCompensated`] collider's pose, oldest
/// first, at most [`capacity`](Self::capacity) of them.
#[derive(Debug, Clone)]
pub struct PoseHistory {
    capacity: usize,
    entries: VecDeque<Entry>,
}

impl PoseHistory {
    /// A ring of `capacity` ticks (at least two: one to interpolate from,
    /// one to).
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(2);
        Self {
            capacity,
            entries: VecDeque::with_capacity(capacity),
        }
    }

    /// A ring covering `history` at a tick of `step` seconds, and the tick
    /// now: `history / step + 1` ticks.
    pub fn covering(history: Duration, step: f32) -> Self {
        let step = f64::from(step.max(1e-4));
        let ticks = (history.as_secs_f64() / step).ceil();
        // A history of hours is a mistake in a config, not a ring to fill.
        Self::new(ticks.clamp(0.0, 1e6) as usize + 1)
    }

    /// How many ticks it keeps.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// How many ticks it holds.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether it holds nothing yet.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The oldest tick held.
    pub fn oldest(&self) -> Option<u32> {
        self.entries.front().map(|e| e.tick)
    }

    /// The newest tick held: "now", for a rewind.
    pub fn newest(&self) -> Option<u32> {
        self.entries.back().map(|e| e.tick)
    }

    /// Forgets every tick: a new session, a new world.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Records where every [`LagCompensated`] entity with a [`Collider`]
    /// and a [`Transform`] is, as `tick`. Call it once a tick, after the
    /// step that moved them. A tick not after the newest held starts the
    /// history again — a session restarted, a clock reset — rather than
    /// muddling two timelines; the oldest tick goes once the ring is full,
    /// its storage reused.
    pub fn record(&mut self, world: &World, tick: u32) {
        if self.newest().is_some_and(|newest| tick <= newest) {
            log::debug!("pose history: tick {tick} is not after the last; starting again");
            self.entries.clear();
        }
        let mut poses = if self.entries.len() >= self.capacity {
            let mut reused = self
                .entries
                .pop_front()
                .map(|e| e.poses)
                .unwrap_or_default();
            reused.clear();
            reused
        } else {
            Vec::new()
        };
        poses.extend(world.query::<LagCompensated>().filter_map(|(entity, _)| {
            let collider = world.get::<Collider>(entity)?;
            let transform = world.get::<Transform>(entity)?;
            Some((entity, *collider, *transform))
        }));
        poses.sort_unstable_by_key(|(entity, _, _)| *entity);
        self.entries.push_back(Entry { tick, poses });
    }

    /// The tick a rewind to `tick` reads: no earlier than the oldest held.
    /// `None` when it reads the world as it is — nothing held, or `tick`
    /// at or past the newest.
    pub fn clamp(&self, tick: f64) -> Option<f64> {
        let (oldest, newest) = (self.oldest()?, self.newest()?);
        if tick.is_nan() || tick >= f64::from(newest) {
            return None;
        }
        Some(tick.max(f64::from(oldest)))
    }

    /// The world as it was at `tick` (fractional): every lag-compensated
    /// collider at its pose then, interpolated between the two ticks
    /// around it, and hidden if it was not there yet. Earlier than the
    /// oldest tick held is the oldest; at or past the newest, or with
    /// nothing held, it is the world as it is ([`Rewound::live`]).
    ///
    /// Bound by the live world: an entity despawned since is not found.
    pub fn rewound<'w>(&self, world: &'w World, tick: f64) -> Rewound<'w> {
        let Some(tick) = self.clamp(tick) else {
            return Rewound::live(world);
        };
        // The newest tick at or before the moment, and the one after it.
        let after = self.entries.partition_point(|e| f64::from(e.tick) <= tick);
        let Some(from) = after.checked_sub(1).and_then(|i| self.entries.get(i)) else {
            return Rewound::live(world);
        };
        let to = self.entries.get(after);
        let t = to.map_or(0.0, |to| {
            let span = f64::from(to.tick) - f64::from(from.tick);
            ((tick - f64::from(from.tick)) / span) as f32
        });

        let mut poses = Poses::default();
        for (entity, _) in world.query::<LagCompensated>() {
            if world.get::<Collider>(entity).is_none() {
                continue;
            }
            let pose = from.find(entity).map(|(_, collider, before)| {
                let at = match to.and_then(|to| to.find(entity)) {
                    Some((_, _, after)) if t > 0.0 => before.interpolate(after, t),
                    _ => *before,
                };
                (*collider, at)
            });
            poses.insert(entity, pose);
        }
        Rewound::past(world, poses, tick)
    }
}

#[cfg(test)]
mod tests;
