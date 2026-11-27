//! The game's say in what each client is sent: the points it watches
//! besides the entities it owns, and what was relevant to it.

use glam::Vec2;
use voltra_ecs::Entity;
use voltra_net::PeerId;

use super::{ReplicationServer, MAX_INTEREST_POINTS};
use crate::Interest;

impl ReplicationServer {
    /// The interest it culls by: the config's, once checked; `None` when
    /// every client is sent everything.
    pub fn interest(&self) -> Option<&Interest> {
        self.interest.as_ref().map(|(interest, _)| interest)
    }

    /// Sets the points `peer` is sent the world around, besides the
    /// entities it owns — a spectator's camera, a scout's sight, a level's
    /// area of play. Replaces the last it was given; kept until replaced or
    /// the peer leaves. At most [`MAX_INTEREST_POINTS`]: the rest are
    /// dropped, with a warning. Without interest management it changes
    /// nothing.
    ///
    /// `false`, and nothing kept, for a peer that is not a client of this
    /// server (not yet joined, or gone): points for it would outlive it.
    pub fn set_interest_points(&mut self, peer: PeerId, mut points: Vec<Vec2>) -> bool {
        if !self.clients.contains_key(&peer) {
            self.points.remove(&peer);
            return false;
        }
        if points.len() > MAX_INTEREST_POINTS {
            log::warn!(
                "{peer}: {} interest points, over the {MAX_INTEREST_POINTS} a client may have; \
                 the first are kept",
                points.len()
            );
            points.truncate(MAX_INTEREST_POINTS);
        }
        if points.is_empty() {
            self.points.remove(&peer);
        } else {
            self.points.insert(peer, points);
        }
        true
    }

    /// Every point `peer` was sent the world around at the last snapshot:
    /// where its entities were, then the game's. Empty without interest
    /// management.
    pub fn interest_points(&self, peer: PeerId) -> &[Vec2] {
        self.clients
            .get(&peer)
            .map_or(&[], |view| view.interest.points.as_slice())
    }

    /// Whether `entity` was relevant to `peer` at the last snapshot: it is
    /// being sent to it. Every replicated entity without interest
    /// management.
    pub fn is_relevant(&self, peer: PeerId, entity: Entity) -> bool {
        let (Some(view), Some(id)) = (self.clients.get(&peer), self.ids.get(&entity)) else {
            return false;
        };
        match self.interest {
            Some(_) => view.interest.relevant.contains(id),
            None => true,
        }
    }
}
