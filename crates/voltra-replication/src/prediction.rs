//! Prediction and reconciliation of the one entity this client owns.
//!
//! Each fixed step the client simulates its own entity with the input it
//! just read — at once, not a round trip later — and sends that input,
//! numbered by the step. The server simulates the same inputs, one a
//! step, and its snapshots say the newest it has. When one brings the
//! entity's server state, the client puts the entity back to it and
//! replays every input the server has not simulated yet: if both ran the
//! same steps, the replay lands exactly where the prediction was, and
//! nothing moves on screen; where they differ, the server wins.
//!
//! Only the one entity is rewound, by the game's own `simulate` — not the
//! world's physics step, which moves every body and cannot be run again
//! for one. That is the determinism this needs, and where it ends:
//!
//! - `simulate` must depend only on the entity's replicated components,
//!   the input and the step: reading another entity (a moving platform,
//!   another player) reads where that one is *now* on the client, not
//!   where it was at the replayed step on the server;
//! - the physics step does not run in the replay, so an entity moved by
//!   its body's velocity, or pushed by contacts, mispredicts and is
//!   corrected. Move a predicted entity kinematically, from `simulate`;
//! - `f32` arithmetic is deterministic for the same operations in the
//!   same order on the same build, but a server compiled for another
//!   architecture may fuse multiplies and adds differently and drift by
//!   an ulp a step — corrected by the next snapshot, invisible at that
//!   size.
//!
//! Unity Netcode's `PredictedGhost`, Overwatch's and Rocket League's
//! command replay, and Gaffer On Games' "client-side prediction" all have
//! this shape; full rollback of the whole world (fighting games) is its
//! own design, and not this.

use std::collections::VecDeque;

use serde::Serialize;
use voltra_ecs::{Entity, World};
use voltra_scene::{ComponentRegistry, Placement};

use crate::wire::{InputTick, NetId};
use crate::ReplicationClient;

/// The most inputs kept for replay: four seconds at 60 a second, far past
/// any round trip a game is playable over.
const MAX_HISTORY: usize = 256;

/// What prediction has done, for a test or a stats overlay.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PredictionStats {
    /// Server states rewound to.
    pub corrections: u64,
    /// Inputs replayed on top of them.
    pub replayed: u64,
    /// How far the last correction moved the entity, after its replay —
    /// zero when the server agreed with the prediction. Its translation's
    /// distance in its own dimension ([`Placement::distance`]): a 2D
    /// entity's in the plane, a 3D one's in space.
    pub last_error: f32,
    /// The largest such distance so far.
    pub max_error: f32,
}

/// Predicts this client's own entity with inputs of type `I`.
///
/// Held by the game, and stepped from its fixed tick — `tick.predict`
/// wires it to the session there.
pub struct Prediction<I> {
    history: VecDeque<(InputTick, I)>,
    tick: InputTick,
    entity: Option<(NetId, Entity)>,
    stats: PredictionStats,
}

impl<I> Default for Prediction<I> {
    fn default() -> Self {
        Self {
            history: VecDeque::new(),
            tick: 0,
            entity: None,
            stats: PredictionStats::default(),
        }
    }
}

impl<I: Serialize> Prediction<I> {
    /// Nothing predicted yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// The entity being predicted, once the server has sent it.
    pub fn entity(&self) -> Option<Entity> {
        self.entity.map(|(_, e)| e)
    }

    /// The last input's number.
    pub fn tick(&self) -> InputTick {
        self.tick
    }

    /// What it has done.
    pub fn stats(&self) -> PredictionStats {
        self.stats
    }

    /// Inputs sent and not yet simulated by the server.
    pub fn pending(&self) -> usize {
        self.history.len()
    }

    /// One fixed step: reconciles with the newest server state if one
    /// came, then simulates `input` on the owned entity and sends it.
    /// Answers the entity, or `None` before the server has sent one this
    /// client owns — the input is then not sent either.
    ///
    /// `simulate(world, entity, input, step)` moves the entity; it is also
    /// what replays, so it must do the same thing every time it is given
    /// the same state and input (see the module's notes).
    pub fn step(
        &mut self,
        world: &mut World,
        client: &mut ReplicationClient,
        registry: &ComponentRegistry,
        input: I,
        step: f32,
        mut simulate: impl FnMut(&mut World, Entity, &I, f32),
    ) -> Option<Entity> {
        let Some((id, entity)) = client.owned() else {
            self.entity = None;
            return None;
        };
        if self.entity != Some((id, entity)) {
            self.entity = Some((id, entity));
            self.history.clear();
            client.set_predicted(Some(id));
        }

        if let Some(correction) = client.take_correction() {
            let before = Placement::of(world, entity);
            for (component, bytes) in &correction.state {
                if let Err(error) = registry.apply_replicated(*component, world, entity, bytes) {
                    log::warn!("correcting {}: {error}", component.name);
                }
            }
            while self
                .history
                .front()
                .is_some_and(|(t, _)| *t <= correction.input_ack)
            {
                self.history.pop_front();
            }
            for (_, input) in &self.history {
                simulate(world, entity, input, step);
            }
            self.stats.corrections += 1;
            self.stats.replayed += self.history.len() as u64;
            if let Some(error) = before.distance(&Placement::of(world, entity)) {
                self.stats.last_error = error;
                self.stats.max_error = self.stats.max_error.max(error);
            }
        } else {
            let acked = client.input_ack();
            while self.history.front().is_some_and(|(t, _)| *t <= acked) {
                self.history.pop_front();
            }
        }

        self.tick = self.tick.wrapping_add(1).max(1);
        simulate(world, entity, &input, step);
        match voltra_net::encode(&input) {
            Ok(bytes) => client.queue_input(self.tick, bytes),
            Err(error) => log::warn!("an input would not encode: {error}"),
        }
        self.history.push_back((self.tick, input));
        while self.history.len() > MAX_HISTORY {
            self.history.pop_front();
        }
        Some(entity)
    }
}
