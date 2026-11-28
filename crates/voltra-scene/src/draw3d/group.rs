//! The seen instances sorted and grouped into instanced draws.

use std::cmp::Ordering;

use voltra_render::render3d::{AlphaClass, Instance3D};

use super::cull::Candidate;
use super::Draw3D;

/// What the seen are sorted by: the order (for the opaque and masked the
/// pipeline above the shader group above the material, for the blended the
/// class alone), the mesh
/// (0 for the blended), the view depth, the entity index — and where the
/// candidate is. 24 bytes, so the sort moves that rather than a whole
/// candidate. A handle's slot alone tells two live materials or meshes
/// apart within one frame, so no generation is needed.
pub(super) type Key = (u64, u32, f32, u32, u32);

/// A run of keys of one pipeline, material and mesh: its nearest depth,
/// and where it starts and ends in the keys.
pub(super) type Run = (f32, usize, usize);

/// The pipeline: the alpha class above the sides above the winding, so
/// opaque sorts before masked before blended.
fn pipeline(candidate: &Candidate) -> u32 {
    (candidate.alpha as u32) << 3
        | u32::from(candidate.double_sided) << 2
        | candidate.winding as u32
}

/// The material's slot, 0 for the default.
fn material(candidate: &Candidate) -> u32 {
    candidate.material.map_or(0, |m| m.index() + 1)
}

/// Which draw a candidate belongs to: pipeline, material, mesh.
fn draw_of(candidate: &Candidate) -> (u32, u32, u32) {
    (
        pipeline(candidate),
        material(candidate),
        candidate.mesh.index(),
    )
}

/// Where the pipeline's bits start in the order: above the shader group
/// (16 bits) above the material's slot (32 bits).
const PIPELINE_SHIFT: u32 = 48;

/// Where the alpha class's bits start: above the sides and the winding.
const CLASS_SHIFT: u32 = PIPELINE_SHIFT + 3;

fn is_blend(key: &Key) -> bool {
    (key.0 >> CLASS_SHIFT) as u32 == AlphaClass::Blend as u32
}

/// Opaque and masked: by pipeline → shader → material → mesh, front to back within
/// a mesh. Blended, after them: back to front, whatever their material.
fn order(a: &Key, b: &Key) -> Ordering {
    a.0.cmp(&b.0).then_with(|| {
        if is_blend(a) {
            b.2.total_cmp(&a.2).then(a.3.cmp(&b.3))
        } else {
            a.1.cmp(&b.1).then(a.2.total_cmp(&b.2)).then(a.3.cmp(&b.3))
        }
    })
}

/// Sorts the seen of `candidates` and writes their draws into `draws` and
/// their instances into `instances`: the opaque and masked first, one draw
/// per pipeline, material and mesh, the draws of one pipeline and material
/// ordered by their nearest instance so the depth test rejects early; then
/// the blended, back to front, a draw per run of one pipeline, material and
/// mesh — what blends over must be drawn after what it blends over. `keys`
/// and `runs` are scratch, kept for their storage.
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
                let blend = c.alpha == AlphaClass::Blend;
                // The blended are ordered by depth alone: their pipeline,
                // material and mesh only split the runs, below.
                let (order, mesh) = if blend {
                    (u64::from(AlphaClass::Blend as u32) << CLASS_SHIFT, 0)
                } else {
                    (
                        u64::from(pipeline(c)) << PIPELINE_SHIFT
                            | u64::from(c.shader) << 32
                            | u64::from(material(c)),
                        c.mesh.index(),
                    )
                };
                (order, mesh, c.depth, c.entity.index(), at as u32)
            }),
    );
    keys.sort_unstable_by(order);
    runs.clear();
    let mut start = 0;
    while start < keys.len() {
        let draw = draw_of(&candidates[keys[start].4 as usize]);
        let end = start
            + keys[start..]
                .iter()
                .take_while(|k| draw_of(&candidates[k.4 as usize]) == draw)
                .count();
        // A blended run's depth is its place in the back-to-front order.
        let depth = if is_blend(&keys[start]) {
            f32::INFINITY
        } else {
            keys[start].2
        };
        runs.push((depth, start, end));
        start = end;
    }
    // Opaque and masked runs: by pipeline and material, then nearest
    // first. Blended runs stay in their back-to-front order, after.
    runs.sort_by(|a, b| {
        let (ka, kb) = (&keys[a.1], &keys[b.1]);
        match (is_blend(ka), is_blend(kb)) {
            (false, false) => {
                ka.0.cmp(&kb.0)
                    .then(a.0.total_cmp(&b.0))
                    .then(a.1.cmp(&b.1))
            }
            _ => is_blend(ka).cmp(&is_blend(kb)).then(a.1.cmp(&b.1)),
        }
    });
    for &(_, start, end) in runs.iter() {
        let first = instances.len() as u32;
        instances.extend(
            keys[start..end]
                .iter()
                .filter_map(|k| candidates[k.4 as usize].instance),
        );
        let head = &candidates[keys[start].4 as usize];
        draws.push(Draw3D {
            mesh: head.mesh,
            material: head.material,
            instances: first..instances.len() as u32,
            winding: head.winding,
        });
    }
}
