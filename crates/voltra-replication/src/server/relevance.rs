//! Each client's relevant entities, once a snapshot: the grid built over
//! the replicated roots, each client's points read against it, and what
//! left its interest culled.
//!
//! Per client the work is its neighbourhood — the cells around its points,
//! what it was sent last time, what it is sent now — never every entity:
//! the snapshot that follows reads only the relevant ones too.

use std::collections::{BTreeMap, HashMap, HashSet};

use glam::Vec2;
use voltra_net::PeerId;

use super::snapshot::Tracked;
use super::view::ClientView;
use crate::{InterestGrid, NetId};

/// Fills every client's [`ClientInterest`](super::view::ClientInterest)
/// for this snapshot from `tracked`, and culls what left.
pub(super) fn update(
    grid: &mut InterestGrid,
    tracked: &BTreeMap<NetId, Tracked>,
    clients: &mut BTreeMap<PeerId, ClientView>,
    extra: &HashMap<PeerId, Vec<Vec2>>,
) {
    grid.clear();
    // Roots relevant to everyone: marked so, or with nowhere to be.
    let mut always: Vec<NetId> = Vec::new();
    let mut children: HashMap<NetId, Vec<NetId>> = HashMap::new();
    let mut owned: HashMap<PeerId, Vec<(NetId, Option<Vec2>)>> = HashMap::new();
    for (&id, entity) in tracked {
        let place = entity.place;
        if place.root != id {
            children.entry(place.root).or_default().push(id);
        } else if let (false, Some(at)) = (place.always, place.at) {
            grid.insert(id, at, place.radius);
        } else {
            always.push(id);
        }
        if let Some(owner) = entity.spawn_value.owner {
            owned
                .entry(PeerId(owner))
                .or_default()
                .push((place.root, place.at));
        }
    }

    for (peer, view) in clients.iter_mut() {
        let interest = &mut view.interest;
        let mine = owned.get(peer).map(Vec::as_slice).unwrap_or_default();
        interest.points.clear();
        interest
            .points
            .extend(mine.iter().filter_map(|(_, at)| *at));
        interest
            .points
            .extend(extra.get(peer).into_iter().flatten().copied());

        let mut roots = HashSet::with_capacity(interest.roots.len());
        grid.relevant(&interest.points, &interest.roots, &mut roots);
        // What it owns is always its own to see, wherever it went.
        roots.extend(mine.iter().map(|(root, _)| *root));
        roots.extend(always.iter().copied());
        interest.roots = roots;

        interest.relevant.clear();
        for &root in &interest.roots {
            // Still being culled: sent again only once the cull is in, or
            // its spawn could overtake it.
            if interest.culled.contains(&root) || !tracked.contains_key(&root) {
                continue;
            }
            interest.relevant.push(root);
            for &child in children.get(&root).into_iter().flatten() {
                if !interest.culled.contains(&child) {
                    interest.relevant.push(child);
                }
            }
        }
        let now: HashSet<NetId> = interest.relevant.iter().copied().collect();
        for id in &interest.relevant {
            // Back: the client consumes what it set aside when the spawn
            // comes.
            interest.out.remove(id);
        }

        let leaving: Vec<NetId> = view
            .entities
            .keys()
            .copied()
            .filter(|id| !now.contains(id))
            .collect();
        for id in leaving {
            let Some(left) = view.entities.remove(&id) else {
                continue;
            };
            // One whose spawn never went has nothing on the client to cull.
            if left.sent_spawn.is_some() {
                view.interest.culled.insert(id);
            }
        }
    }
}
