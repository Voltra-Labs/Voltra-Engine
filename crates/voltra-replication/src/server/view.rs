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

use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::sync::Arc;

use glam::Vec2;

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
    /// The first snapshot this view can be in: an acknowledgement of an
    /// older one is of a copy the client no longer has (it went out of
    /// interest, or the client asked for it whole again).
    pub(crate) born: ServerTick,
}

impl EntityView {
    pub(crate) fn new(components: usize, born: ServerTick) -> Self {
        Self {
            acked_spawn: None,
            sent_spawn: None,
            acked: vec![(0, None); components],
            sent: vec![None; components],
            bank: 0.0,
            born,
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
    pub(crate) culled: Vec<NetId>,
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
    /// The interest this client is sent by (stage 127); all of it unused
    /// without interest management.
    pub(crate) interest: ClientInterest,
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
        for id in sent.culled {
            // Acknowledged: the client has despawned it and set it aside.
            if self.interest.culled.remove(&id) {
                self.interest.out.insert(id);
            }
        }
        for entity in sent.entities {
            // Gone since: its despawn (or its cull) is what the client
            // needs now; or a copy the client has since lost.
            let Some(view) = self.entities.get_mut(&entity.id) else {
                continue;
            };
            if tick < view.born {
                continue;
            }
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
    /// having lost its copy. `born` is the next snapshot's tick.
    pub(crate) fn resync(&mut self, id: NetId, components: usize, born: ServerTick) {
        if let Some(view) = self.entities.get_mut(&id) {
            *view = EntityView::new(components, born);
        }
    }
}

/// What one client is sent by interest management.
#[derive(Debug, Default)]
pub(crate) struct ClientInterest {
    /// The replicated roots relevant to it after the last snapshot: what
    /// hysteresis compares against.
    pub(crate) roots: HashSet<NetId>,
    /// Every entity it is sent this snapshot, roots and their children.
    pub(crate) relevant: Vec<NetId>,
    /// Culls sent, not yet acknowledged: in every snapshot until one is.
    /// An entity waiting here is not sent again — its spawn would race the
    /// cull it follows.
    pub(crate) culled: BTreeSet<NetId>,
    /// Culls acknowledged: entities the client set aside, which a real
    /// despawn must still reach so it lets go of them.
    pub(crate) out: HashSet<NetId>,
    /// The points it was given this snapshot: its own entities' and the
    /// game's.
    pub(crate) points: Vec<Vec2>,
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
        view.entities.insert(1, EntityView::new(1, 0));
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
            culled: vec![],
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
        view.entities.insert(1, EntityView::new(1, 0));
        for (tick, byte) in [(4, 1), (5, 2)] {
            view.sent.push_back(Sent {
                tick,
                entities: vec![SentEntity {
                    id: 1,
                    spawn: None,
                    components: vec![(0, value(byte))],
                }],
                despawned: vec![],
                culled: vec![],
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

    #[test]
    fn an_acknowledged_cull_is_out_and_an_old_ack_misses_a_new_copy() {
        let mut view = ClientView::default();
        view.interest.culled.insert(7);
        view.sent.push_back(Sent {
            tick: 4,
            entities: vec![SentEntity {
                id: 1,
                spawn: None,
                components: vec![(0, value(1))],
            }],
            despawned: vec![],
            culled: vec![7],
        });
        // Entity 1 went and came back since tick 4: a fresh view.
        view.entities.insert(1, EntityView::new(1, 6));
        let mut four = ReceivedTicks::default();
        four.insert(4);
        view.acknowledge(four);
        assert!(view.interest.culled.is_empty());
        assert!(view.interest.out.contains(&7));
        assert_eq!(
            view.entities[&1].acked[0],
            (0, None),
            "tick 4 was a copy the client no longer has"
        );
    }
}
