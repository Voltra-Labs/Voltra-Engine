//! A client's asks for entities to be sent whole again.

use std::collections::BTreeMap;

use super::snapshot::Tracked;
use super::view::ClientView;
use crate::wire::{NetId, ServerTick, MAX_LISTED};

/// Answers `asks` from the client `view` is of: each entity still
/// `tracked` is started over, to be sent whole in the next snapshot after
/// `tick`.
pub(super) fn answer(
    view: &mut ClientView,
    asks: Vec<NetId>,
    tracked: &BTreeMap<NetId, Tracked>,
    components: usize,
    tick: ServerTick,
) {
    for id in asks.into_iter().take(MAX_LISTED) {
        if tracked.contains_key(&id) {
            view.resync(id, components, tick.wrapping_add(1));
        } else {
            // Gone, or never ours: said as a despawn, which is in
            // every snapshot until acknowledged — or the client,
            // whose copy went while its view was being started
            // over (no spawn sent, so no despawn owed), would ask
            // for it for good.
            view.despawned.insert(id);
        }
    }
}
