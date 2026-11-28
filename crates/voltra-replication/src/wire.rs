//! What replication sends, laid out for `postcard`.
//!
//! A snapshot is written by hand from parts — a header, a count, then each
//! entity's update encoded on its own — because the server must know each
//! update's size before it commits to it (the bandwidth budget), and
//! encoding the whole snapshot again per candidate would be quadratic. The
//! bytes are exactly what `postcard` makes of [`Snapshot`], so the client
//! reads it in one call.

use serde::{Deserialize, Serialize};
use voltra_scene::SceneId;

use crate::ViewTime;

/// The version of what this module lays on the wire. **Bump it on any
/// change to a replication message** — a field added to [`Control`],
/// [`SnapshotHeader`], [`EntityUpdate`], [`Spawn`] or [`ClientMessage`],
/// one reordered, a meaning changed: postcard is positional, and a peer
/// built before the change would decode garbage or nothing. It goes into
/// the session's [`NetConfig::layer_version`](voltra_net::NetConfig), which
/// the handshake's fingerprint covers, so such a peer is refused at the
/// door with `ProtocolMismatch`.
///
/// `1`: stage 114's (0.3). `2`: stage 127's interest — the schema's
/// `interest` flag and the snapshot's `culled` list. `3`: stage 128's lag
/// compensation — each input's [`ViewTime`]. `4`: stage 132's (0.5)
/// renames — the schema's component names are the registered ones, and
/// `Transform` became `Transform2D`: a 0.4 peer would connect and then
/// match none of a 0.5 server's transforms, so it is refused instead. `5`:
/// stage 134's — `Transform3D` is in every schema, replicated and
/// interpolated by default: a peer built before would connect and leave
/// every 3D entity where it spawned.
///
/// A change of the registry's names, or of which of them replicate by
/// default, reaches the wire through [`Control::Schema`] and is a bump too.
pub const REPLICATION_WIRE_VERSION: u32 = 5;

/// A replicated entity's number in a session: given by the server, never
/// reused while it runs, and far smaller on the wire than a [`SceneId`].
pub type NetId = u32;

/// A server tick: one fixed step of its simulation.
pub type ServerTick = u32;

/// A client's own tick, which its inputs are numbered by.
pub type InputTick = u32;

/// Reliable, server to client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) enum Control {
    /// The replicated components, by name, in the order snapshots number
    /// them; a server tick's length in seconds; and whether the server
    /// culls by interest — a client then sets aside, rather than loses,
    /// what it is told is out of its interest.
    Schema {
        components: Vec<String>,
        step: f32,
        snapshot_interval: u32,
        interest: bool,
    },
}

/// Everything a client needs to make an entity it has not seen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Spawn {
    /// Its id on the server: a client whose own copy of the level has the
    /// entity adopts it rather than making another.
    pub(crate) id: SceneId,
    /// The prefab it was spawned from, which the client spawns too — the
    /// sprite, the collider, the children — before the replicated
    /// components are laid over it.
    pub(crate) prefab: Option<String>,
    /// The peer it belongs to, if any.
    pub(crate) owner: Option<u32>,
}

/// One entity in a snapshot: what changed since the client last
/// acknowledged it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct EntityUpdate {
    pub(crate) id: NetId,
    /// Until the client has acknowledged a snapshot that carried it.
    pub(crate) spawn: Option<Spawn>,
    /// By schema index: a value, or `None` for a component it no longer
    /// has.
    pub(crate) components: Vec<(u16, Option<Vec<u8>>)>,
}

/// What a snapshot starts with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct SnapshotHeader {
    pub(crate) tick: ServerTick,
    /// The newest of this client's inputs the server has simulated.
    pub(crate) input_ack: InputTick,
    /// Whether every entity with something to send is in it. A client
    /// takes an entity missing from a complete snapshot as unchanged at
    /// its tick — an interpolation sample — and an entity it has that the
    /// server never sent as not the server's.
    pub(crate) complete: bool,
    pub(crate) despawned: Vec<NetId>,
    /// Entities gone out of this client's interest: despawned on it as a
    /// despawn is, and spawned again whole if they come back. Not
    /// destroyed on the server — a later despawn of one says so.
    pub(crate) culled: Vec<NetId>,
}

/// A whole snapshot, as the client reads one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Snapshot {
    pub(crate) tick: ServerTick,
    pub(crate) input_ack: InputTick,
    pub(crate) complete: bool,
    pub(crate) despawned: Vec<NetId>,
    pub(crate) culled: Vec<NetId>,
    pub(crate) updates: Vec<EntityUpdate>,
}

/// Client to server, every frame.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub(crate) struct ClientMessage {
    /// The newest snapshot received (`0`: none yet) and, bit `n`, the one
    /// `n + 1` ticks before it.
    pub(crate) newest: ServerTick,
    pub(crate) mask: u64,
    /// The recent inputs not yet acknowledged, oldest first, each with
    /// the moment the client was drawing remote entities at when it made
    /// it (`None` before its first snapshot) — what a lag-compensated
    /// shot in it is tested against.
    pub(crate) inputs: Vec<(InputTick, Option<ViewTime>, Vec<u8>)>,
    /// Entities a snapshot updated that this client has no copy of — its
    /// game despawned one — to be sent whole again.
    pub(crate) resync: Vec<NetId>,
}

/// The most of anything a client may list in one message: more is a
/// client misbehaving, and the rest is ignored.
pub(crate) const MAX_LISTED: usize = 64;

/// Writes a snapshot from its parts.
pub(crate) fn write_snapshot(header: &SnapshotHeader, updates: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = encode(header);
    bytes.extend(encode(&updates.len()));
    for update in updates {
        bytes.extend_from_slice(update);
    }
    bytes
}

/// `value` encoded. Replication's own types always encode: no map with
/// non-string keys, no unsized sequence.
pub(crate) fn encode<T: Serialize + ?Sized>(value: &T) -> Vec<u8> {
    voltra_net::encode(value).expect("replication's own messages always encode")
}

/// Reads one value and nothing after it.
#[cfg(test)]
pub(crate) fn decode<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
) -> Result<T, voltra_net::CodecError> {
    voltra_net::decode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_snapshot_written_in_parts_reads_whole() {
        let header = SnapshotHeader {
            tick: 300,
            input_ack: 7,
            complete: true,
            despawned: vec![4, 900],
            culled: vec![12],
        };
        let updates = vec![
            EntityUpdate {
                id: 1,
                spawn: Some(Spawn {
                    id: SceneId::new(),
                    prefab: Some("prefabs/ship.prefab.ron".into()),
                    owner: Some(2),
                }),
                components: vec![(0, Some(vec![1, 2, 3])), (2, None)],
            },
            EntityUpdate {
                id: 70_000,
                spawn: None,
                components: vec![],
            },
        ];
        let parts: Vec<Vec<u8>> = updates.iter().map(encode).collect();
        let bytes = write_snapshot(&header, &parts);
        let read: Snapshot = decode(&bytes).expect("reads");
        assert_eq!(read.tick, 300);
        assert_eq!(read.input_ack, 7);
        assert!(read.complete);
        assert_eq!(read.despawned, vec![4, 900]);
        assert_eq!(read.culled, vec![12]);
        assert_eq!(read.updates, updates);
    }

    #[test]
    fn junk_is_an_error() {
        let mut seed = 0x9e37_79b9_u32;
        for len in 0..512 {
            let bytes: Vec<u8> = (0..len % 48)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    seed as u8
                })
                .collect();
            let _ = decode::<Snapshot>(&bytes);
            let _ = decode::<ClientMessage>(&bytes);
            let _ = decode::<Control>(&bytes);
        }
    }
}
