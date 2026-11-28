//! The seen instances sorted and grouped into instanced draws.

use super::cull::Candidate;
use super::Draw3D;
use voltra_render::render3d::Instance3D;

/// What the seen are sorted by: mesh slot and generation, view depth,
/// entity index — and where the candidate is. Small, so the sort moves
/// 24 bytes an element rather than a whole candidate.
type Key = (u32, u32, f32, u32, u32);

/// Sorts the seen of `candidates` by mesh, then front to back, and writes
/// one draw per mesh into `draws` and their instances into `instances`;
/// the draws ordered by their nearest instance. `keys` is scratch, kept
/// for its storage.
pub(super) fn group(
    candidates: &[Candidate],
    keys: &mut Vec<Key>,
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
                    c.mesh.index(),
                    c.mesh.generation(),
                    c.depth,
                    c.entity.index(),
                    at as u32,
                )
            }),
    );
    keys.sort_unstable_by(|a, b| {
        (a.0, a.1)
            .cmp(&(b.0, b.1))
            .then(a.2.total_cmp(&b.2))
            .then(a.3.cmp(&b.3))
    });
    // Runs of one mesh, each with its nearest depth.
    let mut runs: Vec<(f32, usize, usize)> = Vec::new();
    let mut start = 0;
    while start < keys.len() {
        let mesh = (keys[start].0, keys[start].1);
        let end = start
            + keys[start..]
                .iter()
                .take_while(|k| (k.0, k.1) == mesh)
                .count();
        runs.push((keys[start].2, start, end));
        start = end;
    }
    runs.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    for (_, start, end) in runs {
        let first = instances.len() as u32;
        instances.extend(
            keys[start..end]
                .iter()
                .filter_map(|k| candidates[k.4 as usize].instance),
        );
        draws.push(Draw3D {
            mesh: candidates[keys[start].4 as usize].mesh,
            instances: first..instances.len() as u32,
        });
    }
}
