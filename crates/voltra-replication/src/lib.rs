//! Replication: the server's world, sent to its clients.
//!
//! Above `voltra-net`'s connections and beside the scene: components a game
//! registers as replicated ([`ComponentRegistry::replicate`]) on entities
//! marked [`Replicated`] are sent by the server — spawned, changed and
//! despawned — and a client's world follows. Five parts, one per stage of
//! the plan:
//!
//! - **Snapshots and deltas** ([`ReplicationServer`], stage 114): a
//!   snapshot per tick per client, carrying only what differs from what
//!   that client acknowledged, the most important first within a
//!   per-client byte budget.
//! - **Interest** ([`Interest`], stage 127): each client sent only what is
//!   near the entities it owns or the points the game watches for it, on
//!   a grid ([`InterestGrid`]); what leaves is despawned on it, what comes
//!   back spawned whole.
//! - **Interpolation** ([`ReplicationClient::interpolate`], [`Timeline`],
//!   stage 115): remote entities drawn between the two snapshots around a
//!   moment a jitter buffer behind the newest, the buffer sized from the
//!   jitter measured.
//! - **Prediction** ([`Prediction`], stage 116): the entity this client
//!   owns ([`NetOwner`]) simulated ahead with the player's inputs, put back
//!   to the server's state and replayed when that state arrives.
//! - **Lag compensation** ([`LagCompensation`], stage 128): each input
//!   carries the moment its client was drawing ([`ViewTime`]), which the
//!   server's hit tests rewind to, within a bound.
//!
//! The engine's loop drives all of it through `tick.net` (`voltra-core`);
//! this crate is the part that knows neither a window nor a frame.
//!
//! Untrusted input: a snapshot or a client message that will not decode is
//! dropped whole; a client's lists are cut at a fixed length, its inputs
//! at [`ReplicationConfig::max_input_size`] and
//! [`ReplicationConfig::max_buffered_inputs`]; a component value that will
//! not decode as its type changes nothing.
//!
//! [`ComponentRegistry::replicate`]: voltra_scene::ComponentRegistry::replicate
//! [`Replicated`]: voltra_scene::Replicated

#![warn(missing_docs)]

mod acks;
mod client;
mod config;
mod interest;
mod lag;
mod owner;
mod prediction;
mod server;
mod timeline;
mod wire;

pub use client::{Correction, ReplicaStats, ReplicationClient};
pub use config::{ReplicationChannels, ReplicationConfig};
pub use interest::{Interest, InterestGrid, MAX_CELLS_PER_AXIS};
pub use lag::{LagCompensation, ViewTime};
pub use owner::NetOwner;
pub use prediction::{Prediction, PredictionStats};
pub use server::{ClientReplicationStats, ReplicationServer, MAX_INTEREST_POINTS};
pub use timeline::Timeline;
pub use wire::{InputTick, NetId, ServerTick, REPLICATION_WIRE_VERSION};

/// Replication for one session: the server's half or a client's.
pub enum Replication {
    /// This end hosts: it sends snapshots.
    Server(Box<ReplicationServer>),
    /// This end joined: it applies them.
    Client(Box<ReplicationClient>),
}

impl Replication {
    /// The server's half, if this end hosts.
    pub fn server(&self) -> Option<&ReplicationServer> {
        match self {
            Self::Server(server) => Some(server),
            Self::Client(_) => None,
        }
    }

    /// The server's half, mutably.
    pub fn server_mut(&mut self) -> Option<&mut ReplicationServer> {
        match self {
            Self::Server(server) => Some(server),
            Self::Client(_) => None,
        }
    }

    /// The client's half, if this end joined.
    pub fn client(&self) -> Option<&ReplicationClient> {
        match self {
            Self::Client(client) => Some(client),
            Self::Server(_) => None,
        }
    }

    /// The client's half, mutably.
    pub fn client_mut(&mut self) -> Option<&mut ReplicationClient> {
        match self {
            Self::Client(client) => Some(client),
            Self::Server(_) => None,
        }
    }
}
