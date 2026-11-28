//! The seen instances sorted and grouped into instanced draws.

use super::cull::Candidate;
use super::Draw3D;
use voltra_render::render3d::Instance3D;

/// Sorts the seen of `candidates` by mesh, then front to back, and writes
/// one draw per mesh into `draws` and their instances into `instances`;
/// the draws ordered by their nearest instance.
pub(super) fn group(
    candidates: &mut [Candidate],
    instances: &mut Vec<Instance3D>,
    draws: &mut Vec<Draw3D>,
) {
    candidates.sort_unstable_by(|a, b| {
        let key = |c: &Candidate| (c.instance.is_none(), c.mesh.index(), c.mesh.generation());
        key(a)
            .cmp(&key(b))
            .then(a.depth.total_cmp(&b.depth))
            .then(a.entity.index().cmp(&b.entity.index()))
    });
    // Runs of one mesh, each with its nearest depth.
    let mut runs: Vec<(f32, usize, usize)> = Vec::new();
    let seen = candidates
        .iter()
        .take_while(|c| c.instance.is_some())
        .count();
    let mut start = 0;
    while start < seen {
        let mesh = candidates[start].mesh;
        let end = start
            + candidates[start..seen]
                .iter()
                .take_while(|c| c.mesh == mesh)
                .count();
        runs.push((candidates[start].depth, start, end));
        start = end;
    }
    runs.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    for (_, start, end) in runs {
        let first = instances.len() as u32;
        instances.extend(candidates[start..end].iter().filter_map(|c| c.instance));
        draws.push(Draw3D {
            mesh: candidates[start].mesh,
            instances: first..instances.len() as u32,
        });
    }
}
