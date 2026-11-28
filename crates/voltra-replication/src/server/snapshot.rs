//! One client's snapshot: what is due, most important first, as much as
//! its budget holds.
//!
//! Priorities are banked, as in Gaffer On Games' priority accumulator and
//! Unreal's `NetPriority` scaled by the time since an actor was last sent:
//! each snapshot an entity has something due, its priority is added to its
//! bank; the snapshot takes the largest banks first; an entity sent starts
//! from nothing again. A low-priority entity waits longer but never
//! forever, which a plain sort by priority would not promise.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::view::{ClientView, EntityView, Sent, SentEntity, Value};
use crate::wire::{self, EntityUpdate, NetId, ServerTick, SnapshotHeader, Spawn};

/// Despawns one snapshot lists at most; the rest wait for the next.
const MAX_DESPAWNS: usize = 128;

/// A varint count of updates takes at most this many bytes here.
const COUNT_BYTES: usize = 3;

/// An entity as the server has it this tick.
#[derive(Debug)]
pub(crate) struct Tracked {
    pub(crate) entity: voltra_ecs::Entity,
    /// Its [`Spawn`], and the same encoded, which is what is compared.
    pub(crate) spawn: Arc<[u8]>,
    pub(crate) spawn_value: Spawn,
    pub(crate) components: Vec<Value>,
    pub(crate) priority: f32,
    /// Where it stands for interest management, read only with it on.
    pub(crate) place: Place,
}

/// Where an entity stands for interest management.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Place {
    /// Its replicated root: the topmost replicated entity above it, or
    /// itself. A child is relevant exactly when its root is, so a client
    /// never has half a hierarchy.
    pub(crate) root: NetId,
    /// Its position in the world, in the dimension of its transform — a
    /// `Transform2D`'s, else a `Transform3D`'s — if it has one.
    pub(crate) at: Option<crate::InterestPoint>,
    /// [`Replicated::always_relevant`](voltra_scene::Replicated).
    pub(crate) always: bool,
    /// [`Replicated::relevance_radius`](voltra_scene::Replicated).
    pub(crate) radius: Option<f32>,
}

/// Builds `view`'s snapshot for `tick`, and notes in `view` what it sent.
/// `relevant` is what interest management lets it be sent — `None`, all of
/// `tracked`; an id there that is not tracked is passed over.
pub(crate) fn build(
    view: &mut ClientView,
    tracked: &BTreeMap<NetId, Tracked>,
    relevant: Option<&[NetId]>,
    tick: ServerTick,
    budget: usize,
) -> Vec<u8> {
    // The real despawns first, then the answers to asks for entities gone.
    let despawned: Vec<NetId> = view
        .despawned
        .iter()
        .filter(|id| !view.asked_gone.contains(id))
        .chain(view.asked_gone.iter())
        .copied()
        .take(MAX_DESPAWNS)
        .collect();
    let culled: Vec<NetId> = view
        .interest
        .culled
        .iter()
        .copied()
        .take(MAX_DESPAWNS)
        .collect();
    let mut header = SnapshotHeader {
        tick,
        input_ack: view.inputs.consumed,
        complete: false,
        despawned,
        culled,
    };
    let mut room = budget.saturating_sub(wire::encode(&header).len() + COUNT_BYTES);

    // What is due, and the bank it waits with.
    let mut due: Vec<(NetId, f32)> = Vec::new();
    let candidates: Box<dyn Iterator<Item = (NetId, &Tracked)>> = match relevant {
        Some(ids) => Box::new(
            ids.iter()
                .filter_map(|id| tracked.get(id).map(|entity| (*id, entity))),
        ),
        None => Box::new(tracked.iter().map(|(id, entity)| (*id, entity))),
    };
    for (id, entity) in candidates {
        let components = entity.components.len();
        let seen = view
            .entities
            .entry(id)
            .or_insert_with(|| EntityView::new(components, tick));
        let any = seen.spawn_due(&entity.spawn)
            || entity
                .components
                .iter()
                .enumerate()
                .any(|(index, value)| seen.component_due(index, value));
        if any {
            seen.bank += entity.priority.max(0.0);
            due.push((id, seen.bank));
        }
    }
    due.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));

    let mut parts = Vec::new();
    let mut sent = Sent {
        tick,
        entities: Vec::new(),
        despawned: header.despawned.clone(),
        culled: header.culled.clone(),
    };
    let mut left_out = 0usize;
    for (id, _) in due {
        let entity = &tracked[&id];
        let seen = view
            .entities
            .get_mut(&id)
            .expect("every tracked entity was given a view above");
        let spawn = seen.spawn_due(&entity.spawn).then(|| entity.spawn.clone());
        let changed: Vec<(usize, Value)> = entity
            .components
            .iter()
            .enumerate()
            .filter(|(index, value)| seen.component_due(*index, value))
            .map(|(index, value)| (index, value.clone()))
            .collect();
        let update = EntityUpdate {
            id,
            spawn: spawn.is_some().then(|| entity.spawn_value.clone()),
            components: changed
                .iter()
                .map(|(index, value)| (*index as u16, value.as_ref().map(|v| v.to_vec())))
                .collect(),
        };
        let bytes = wire::encode(&update);
        let alone_too_big = parts.is_empty() && bytes.len() > room;
        if bytes.len() > room && !alone_too_big {
            left_out += 1;
            continue;
        }
        if alone_too_big {
            // Would never fit: sent alone rather than starved forever.
            log::warn!(
                "entity {id}'s update is {} bytes, over the {budget}-byte snapshot budget; sent alone",
                bytes.len()
            );
        }
        room = room.saturating_sub(bytes.len());
        parts.push(bytes);
        seen.bank = 0.0;
        if let Some(spawn) = &spawn {
            seen.sent_spawn = Some(spawn.clone());
        }
        for (index, value) in &changed {
            seen.sent[*index] = value.clone();
        }
        sent.entities.push(SentEntity {
            id,
            spawn,
            components: changed,
        });
        if alone_too_big {
            room = 0;
        }
    }

    header.complete = left_out == 0
        && view.despawned.len() == header.despawned.len()
        && view.interest.culled.len() == header.culled.len();
    view.last_complete = header.complete;
    view.sent.push_back(sent);
    let bytes = wire::write_snapshot(&header, &parts);
    view.last_snapshot_bytes = bytes.len();
    bytes
}
