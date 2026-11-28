//! The game's say in what each client is sent: the points it watches
//! besides the entities it owns, and what was relevant to it.

use voltra_ecs::Entity;
use voltra_net::PeerId;

use super::{ReplicationServer, MAX_INTEREST_POINTS};
use crate::{Interest, InterestPoint};

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
    /// Each is an [`InterestPoint`] — a `Vec2` or a `Vec3` converts — and
    /// sees only entities of its own dimension: a 3D spectator camera
    /// passes `Vec3`s, a 2D one `Vec2`s, a game with both worlds both.
    ///
    /// `false`, and nothing kept, for a peer that is not a client of this
    /// server (not yet joined, or gone): points for it would outlive it.
    /// An empty list clears them; `vec![]` names no point type to infer, so
    /// [`clear_interest_points`](Self::clear_interest_points) says it.
    pub fn set_interest_points<P: Into<InterestPoint>>(
        &mut self,
        peer: PeerId,
        points: impl IntoIterator<Item = P>,
    ) -> bool {
        if !self.clients.contains_key(&peer) {
            self.points.remove(&peer);
            return false;
        }
        // One past the cap, to know it was passed without collecting
        // whatever an iterator would give.
        let mut points: Vec<InterestPoint> = points
            .into_iter()
            .take(MAX_INTEREST_POINTS + 1)
            .map(Into::into)
            .collect();
        if points.len() > MAX_INTEREST_POINTS {
            log::warn!(
                "{peer}: more than the {MAX_INTEREST_POINTS} interest points a client may have; \
                 the first {MAX_INTEREST_POINTS} are kept"
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

    /// Drops the points the game set for `peer`: it is sent the world
    /// around the entities it owns only. `false` for a peer that is not a
    /// client of this server, as [`set_interest_points`](Self::set_interest_points).
    pub fn clear_interest_points(&mut self, peer: PeerId) -> bool {
        self.set_interest_points(peer, std::iter::empty::<InterestPoint>())
    }

    /// Every point `peer` was sent the world around at the last snapshot:
    /// where its entities were, then the game's. Empty without interest
    /// management.
    pub fn interest_points(&self, peer: PeerId) -> &[InterestPoint] {
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
