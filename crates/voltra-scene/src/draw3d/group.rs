//! The seen instances sorted and grouped into instanced draws.

use super::cull::Candidate;
use super::Draw3D;
use voltra_render::render3d::Instance3D;

/// What the seen are sorted by: mirrored or not, mesh slot and generation,
/// view depth, entity index — and where the candidate is. 24 bytes, so the
/// sort moves that rather than a whole candidate.
pub(super) type Key = (u32, u32, u32, f32, u32, u32);

/// A run of keys of one mesh and one winding: its nearest depth, and where
/// it starts and ends in the keys.
pub(super) type Run = (f32, usize, usize);

/// Sorts the seen of `candidates` by winding and mesh, then front to back,
/// and writes one draw per mesh and winding into `draws` and their
/// instances into `instances`; the draws ordered by their nearest
/// instance. `keys` and `runs` are scratch, kept for their storage.
pub(super) fn group(
    candidates: &[Candidate],
    keys: &mut Vec<Key>,
    runs: &mut Vec<Run>,
    instances: &mut Vec<Instance3D>,
    draws: &mut Vec<Draw3D>,
) {
    keys.clear();
    keys.extend(
        candidates
            .iter()
            .enumerate()
            .filter(|(_, c)| c.instance.is_some())
            .map(|(at, c)| {
                (
                    u32::from(c.mirrored),
                    c.mesh.index(),
                    c.mesh.generation(),
                    c.depth,
                    c.entity.index(),
                    at as u32,
                )
            }),
    );
    keys.sort_unstable_by(|a, b| {
        (a.0, a.1, a.2)
            .cmp(&(b.0, b.1, b.2))
            .then(a.3.total_cmp(&b.3))
            .then(a.4.cmp(&b.4))
    });
    runs.clear();
    let mut start = 0;
    while start < keys.len() {
        let group = (keys[start].0, keys[start].1, keys[start].2);
        let end = start
            + keys[start..]
                .iter()
                .take_while(|k| (k.0, k.1, k.2) == group)
                .count();
        runs.push((keys[start].3, start, end));
        start = end;
    }
    runs.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    for &(_, start, end) in runs.iter() {
        let first = instances.len() as u32;
        instances.extend(
            keys[start..end]
                .iter()
                .filter_map(|k| candidates[k.5 as usize].instance),
        );
        let head = &candidates[keys[start].5 as usize];
        draws.push(Draw3D {
            mesh: head.mesh,
            instances: first..instances.len() as u32,
            mirrored: head.mirrored,
        });
    }
}
