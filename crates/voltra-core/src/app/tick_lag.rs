//! Lag compensation through `tick.net`: the server remembers where its
//! `LagCompensated` colliders were each tick, and a game tests a client's
//! shot against the world as that client saw it.
//!
//! The loop records the poses after every fixed step, as the server's
//! tick (the tick its snapshot carries). In a fixed tick the game asks
//! [`Net::rewound`] for the world as `peer` was drawing it when it made
//! this step's input — the [`ViewTime`] the input carried, bounded by
//! [`LagCompensation::target`] — and runs its ray or shape cast there.
//!
//! [`ViewTime`]: voltra_replication::ViewTime
//! [`LagCompensation::target`]: voltra_replication::LagCompensation::target

use voltra_ecs::World;
use voltra_net::PeerId;
use voltra_physics::{PoseHistory, Rewound};
use voltra_replication::{LagCompensation, Replication, ReplicationServer};
use voltra_scene::Collider;

use super::tick_net::Net;

impl Net {
    /// The server's replication, its pose history and its bounds, while
    /// this end is a replicating server with lag compensation on.
    fn lag_parts(&self) -> Option<(&ReplicationServer, &PoseHistory, LagCompensation)> {
        let server = self.replication.as_ref()?.server()?;
        let lag = server.config().lag_compensation?;
        Some((server, self.lag.as_ref()?, lag))
    }

    /// The tick `peer`'s shot this fixed step is tested at: the moment it
    /// reported drawing when it made this step's input, no later than the
    /// newest tick held and no further back than
    /// [`rewind_allowed`](Self::rewind_allowed). The newest tick for a
    /// peer with no input this step (or whose input came before its first
    /// snapshot) — the world as it is. `None` off a replicating server,
    /// with lag compensation off, or before the first step.
    pub fn rewind_target(&self, peer: PeerId) -> Option<f64> {
        let (server, history, lag) = self.lag_parts()?;
        let newest = f64::from(history.newest()?);
        let allowed = self.rewind_allowed(peer).unwrap_or_default();
        Some(lag.target(server.view_time(peer), newest, server.step(), allowed))
    }

    /// How far back `peer` may be rewound this step: what the lag this
    /// server measured for it allows
    /// ([`LagCompensation::allowed`](voltra_replication::LagCompensation::allowed)
    /// of its round trip, the snapshot interval and its input's wait), at
    /// most `max_rewind`. `None` for a peer that is not a client of this
    /// replicating server — the listen host sees the world as it is.
    pub fn rewind_allowed(&self, peer: PeerId) -> Option<std::time::Duration> {
        let (server, _, lag) = self.lag_parts()?;
        let link = self.endpoint.as_ref()?.stats(peer)?;
        Some(lag.allowed(&server.measured_lag(peer, link)?))
    }

    /// The world as `peer` saw it when it made this step's input: its
    /// lag-compensated colliders at [`rewind_target`](Self::rewind_target),
    /// interpolated between the two server ticks around it, and every
    /// replicated collider out of `peer`'s interest hidden — what a client
    /// was not sent it cannot have aimed at. Everything not marked `LagCompensated` is queried
    /// where it is now. Read it in the fixed tick, where
    /// [`input`](Self::input) is this step's.
    ///
    /// The world as it is (still with what `peer` was not sent hidden)
    /// when there is nothing to rewind to: off a server, lag compensation
    /// off, no input from `peer` this step — and for the listen server's
    /// own player, [`PeerId::SERVER`], who sees the world as it is.
    pub fn rewound<'w>(&self, world: &'w World, peer: PeerId) -> Rewound<'w> {
        let mut rewound = match (self.lag_parts(), self.rewind_target(peer)) {
            (Some((_, history, _)), Some(target)) => history.rewound(world, target),
            _ => Rewound::live(world),
        };
        if let Some(server) = self.replication.as_ref().and_then(Replication::server) {
            hide_unsent(server, peer, &mut rewound);
        }
        rewound
    }

    /// The world at `tick` (fractional server ticks): no later than the
    /// newest tick held, no earlier than
    /// [`max_rewind`](LagCompensation::max_rewind) before it nor than the
    /// oldest held. For a game's own rewinds — a shot it timed itself;
    /// a client's goes through [`rewound`](Self::rewound). The world as it
    /// is off a replicating server or with lag compensation off.
    pub fn rewound_to<'w>(&self, world: &'w World, tick: f64) -> Rewound<'w> {
        let Some((server, history, lag)) = self.lag_parts() else {
            return Rewound::live(world);
        };
        let Some(newest) = history.newest().map(f64::from) else {
            return Rewound::live(world);
        };
        let reported = voltra_replication::ViewTime::from_ticks(tick);
        // A moment no tick can name is not the past: the world as it is.
        match reported {
            Some(view) => history.rewound(
                world,
                lag.target(Some(view), newest, server.step(), lag.max_rewind),
            ),
            None => Rewound::live(world),
        }
    }

    /// The server's pose history, while lag compensation runs.
    pub fn pose_history(&self) -> Option<&PoseHistory> {
        self.lag_parts().map(|(_, history, _)| history)
    }

    /// Records where the lag-compensated colliders are, as the server's
    /// tick. After each fixed step.
    pub(super) fn record_poses(&mut self, world: &World) {
        let Some(tick) = self
            .replication
            .as_ref()
            .and_then(Replication::server)
            .map(ReplicationServer::tick)
        else {
            return;
        };
        if let Some(history) = self.lag.as_mut() {
            let _record = voltra_profile::scope("lag compensation");
            history.record(world, tick);
        }
    }
}

/// Hides from `rewound` every collider the server replicates that was not
/// relevant to `peer` at the last snapshot — lag-compensated or not, the
/// client had no copy of it to aim at. Nothing is hidden from a peer that
/// is not one of the server's clients (the listen host), nor without
/// interest management, where every client is sent everything. What is
/// not replicated (the level's walls, loaded on every end) stays.
fn hide_unsent(server: &ReplicationServer, peer: PeerId, rewound: &mut Rewound<'_>) {
    if server.interest().is_none() || server.stats(peer).is_none() {
        return;
    }
    let world = rewound.world();
    let unsent: Vec<_> = world
        .query::<Collider>()
        .map(|(entity, _)| entity)
        .filter(|&entity| server.net_id(entity).is_some() && !server.is_relevant(peer, entity))
        .collect();
    for entity in unsent {
        rewound.hide(entity);
    }
}
