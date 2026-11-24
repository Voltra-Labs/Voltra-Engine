//! What the server knows one client has: per entity, the last value it
//! acknowledged and the last value sent, per component.
//!
//! The delta rule. A component goes in a snapshot when its value differs
//! from the one the client acknowledged *or* from the one last sent. The
//! first half is "deltas against the last acknowledged snapshot": a lost
//! snapshot's change is sent again, because it was never acknowledged. The
//! second covers a value that went back to the acknowledged one while a
//! different one was in flight — without it, the client could keep the
//! in-flight value forever. Values travel whole, so a client applies each
//! update on its own, with no baseline of its own to look up: Unreal's
//! per-connection shadow state compares against the last sent value and
//! resends what a lost packet carried, which is the same rule seen from the
//! other side.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

use super::inputs::InputQueue;
use crate::acks::ReceivedTicks;
use crate::wire::{NetId, ServerTick};

/// A component's value as replication compares it: encoded, or `None` for
/// absent. Shared between the world's copy and every view's.
pub(crate) type Value = Option<Arc<[u8]>>;

/// One entity, as one client knows it.
#[derive(Debug)]
pub(crate) struct EntityView {
    /// The spawn the client acknowledged, and when.
    pub(crate) acked_spawn: Option<(ServerTick, Arc<[u8]>)>,
    pub(crate) sent_spawn: Option<Arc<[u8]>>,
    /// Per schema component: the value acknowledged and its tick (absent at
    /// tick 0 to begin with — a client that has not spawned the entity has
    /// none of its components), and the value last sent.
    pub(crate) acked: Vec<(ServerTick, Value)>,
    pub(crate) sent: Vec<Value>,
    /// Priority banked while it waited for room in a snapshot.
    pub(crate) bank: f32,
}

impl EntityView {
    pub(crate) fn new(components: usize) -> Self {
        Self {
            acked_spawn: None,
            sent_spawn: None,
            acked: vec![(0, None); components],
            sent: vec![None; components],
            bank: 0.0,
        }
    }

    /// Whether the spawn must be (re)sent.
    pub(crate) fn spawn_due(&self, spawn: &Arc<[u8]>) -> bool {
        self.acked_spawn.as_ref().map(|(_, s)| s) != Some(spawn)
            || self.sent_spawn.as_ref() != Some(spawn)
    }

    /// Whether component `index`, now `value`, must be sent.
    pub(crate) fn component_due(&self, index: usize, value: &Value) -> bool {
        self.acked.get(index).map(|(_, v)| v) != Some(value) || self.sent.get(index) != Some(value)
    }
}

/// What one snapshot carried, kept until it is acknowledged or too old.
#[derive(Debug)]
pub(crate) struct Sent {
    pub(crate) tick: ServerTick,
    pub(crate) entities: Vec<SentEntity>,
    pub(crate) despawned: Vec<NetId>,
}

/// One entity's part of a [`Sent`].
#[derive(Debug)]
pub(crate) struct SentEntity {
    pub(crate) id: NetId,
    pub(crate) spawn: Option<Arc<[u8]>>,
    pub(crate) components: Vec<(usize, Value)>,
}

/// Everything the server keeps for one client.
#[derive(Debug, Default)]
pub(crate) struct ClientView {
    pub(crate) schema_sent: bool,
    pub(crate) entities: BTreeMap<NetId, EntityView>,
    /// Despawns not yet acknowledged: in every snapshot until one is.
    pub(crate) despawned: BTreeSet<NetId>,
    pub(crate) sent: VecDeque<Sent>,
    pub(crate) inputs: InputQueue,
    /// The bytes of the last snapshot sent, for a profiler or a test.
    pub(crate) last_snapshot_bytes: usize,
    /// Whether the last snapshot held everything there was to send.
    pub(crate) last_complete: bool,
}

impl ClientView {
    /// Takes in the client's report of which snapshots arrived: each
    /// acknowledged snapshot's values become what the client has, unless
    /// a newer acknowledgement already said so.
    pub(crate) fn acknowledge(&mut self, received: ReceivedTicks) {
        let ticks: BTreeSet<ServerTick> = received.ticks().collect();
        if ticks.is_empty() {
            return;
        }
        let mut kept = VecDeque::with_capacity(self.sent.len());
        for sent in std::mem::take(&mut self.sent) {
            if ticks.contains(&sent.tick) {
                self.apply(sent);
            } else {
                kept.push_back(sent);
            }
        }
        self.sent = kept;
    }

    fn apply(&mut self, sent: Sent) {
        let tick = sent.tick;
        for id in sent.despawned {
            self.despawned.remove(&id);
        }
        for entity in sent.entities {
            // Gone since: its despawn is what the client needs now.
            let Some(view) = self.entities.get_mut(&entity.id) else {
                continue;
            };
            if let Some(spawn) = entity.spawn {
                if view.acked_spawn.as_ref().is_none_or(|(t, _)| *t < tick) {
                    view.acked_spawn = Some((tick, spawn));
                }
            }
            for (index, value) in entity.components {
                if let Some(acked) = view.acked.get_mut(index) {
                    if acked.0 < tick {
                        *acked = (tick, value);
                    }
                }
            }
        }
    }

    /// Starts over for `id`, as if the client had never seen it: it asked,
    /// having lost its copy.
    pub(crate) fn resync(&mut self, id: NetId, components: usize) {
        if let Some(view) = self.entities.get_mut(&id) {
            *view = EntityView::new(components);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(byte: u8) -> Value {
        Some(Arc::from(vec![byte]))
    }

    #[test]
    fn a_value_is_due_until_acknowledged_and_again_when_it_changes() {
        let mut view = ClientView::default();
        view.entities.insert(1, EntityView::new(1));
        let entity = &view.entities[&1];
        assert!(entity.component_due(0, &value(5)));
        assert!(!entity.component_due(0, &None), "absent, as acknowledged");

        // Sent at tick 3: still due (not acknowledged) — lost, it is resent.
        let entity = view.entities.get_mut(&1).expect("there");
        entity.sent[0] = value(5);
        view.sent.push_back(Sent {
            tick: 3,
            entities: vec![SentEntity {
                id: 1,
                spawn: None,
                components: vec![(0, value(5))],
            }],
            despawned: vec![],
        });
        assert!(view.entities[&1].component_due(0, &value(5)));

        let mut received = ReceivedTicks::default();
        received.insert(3);
        view.acknowledge(received);
        assert!(!view.entities[&1].component_due(0, &value(5)));
        assert!(view.entities[&1].component_due(0, &value(6)));
        assert!(view.sent.is_empty());
    }

    #[test]
    fn an_older_acknowledgement_does_not_undo_a_newer_one() {
        let mut view = ClientView::default();
        view.entities.insert(1, EntityView::new(1));
        for (tick, byte) in [(4, 1), (5, 2)] {
            view.sent.push_back(Sent {
                tick,
                entities: vec![SentEntity {
                    id: 1,
                    spawn: None,
                    components: vec![(0, value(byte))],
                }],
                despawned: vec![],
            });
        }
        let mut five = ReceivedTicks::default();
        five.insert(5);
        view.acknowledge(five);
        let mut four = ReceivedTicks::default();
        four.insert(4);
        view.acknowledge(four);
        assert_eq!(view.entities[&1].acked[0], (5, value(2)));
    }
}
