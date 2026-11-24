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
}

/// Builds `view`'s snapshot for `tick`, and notes in `view` what it sent.
pub(crate) fn build(
    view: &mut ClientView,
    tracked: &BTreeMap<NetId, Tracked>,
    tick: ServerTick,
    budget: usize,
) -> Vec<u8> {
    let despawned: Vec<NetId> = view.despawned.iter().copied().take(MAX_DESPAWNS).collect();
    let mut header = SnapshotHeader {
        tick,
        input_ack: view.inputs.consumed,
        complete: false,
        despawned,
    };
    let mut room = budget.saturating_sub(wire::encode(&header).len() + COUNT_BYTES);

    // What is due, and the bank it waits with.
    let mut due: Vec<(NetId, f32)> = Vec::new();
    for (&id, entity) in tracked {
        let components = entity.components.len();
        let seen = view
            .entities
            .entry(id)
            .or_insert_with(|| EntityView::new(components));
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

    header.complete = left_out == 0 && view.despawned.len() == header.despawned.len();
    view.last_complete = header.complete;
    view.sent.push_back(sent);
    let bytes = wire::write_snapshot(&header, &parts);
    view.last_snapshot_bytes = bytes.len();
    bytes
}
