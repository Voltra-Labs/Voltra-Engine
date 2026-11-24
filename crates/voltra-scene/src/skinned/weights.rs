//! Which bones hold which vertices: a first guess by distance, then a brush.
//!
//! The guess is Unity's *Auto Geometry* weights in spirit — each vertex
//! held by its nearest bones, more by the nearer, `1 / d²`, among those no
//! more than three times as far as the nearest: a vertex on a bone is
//! wholly that bone's, one at a joint is shared, and the share fades over
//! the joint's neighbourhood rather than snapping. The
//! brush adds (or takes away) one bone's weight under it with a soft edge,
//! and every touched vertex is renormalised and kept to its four strongest.

use voltra_render::glam::Vec2;

use super::{SkinMesh, MAX_INFLUENCES};

/// How many times the nearest bone's distance another bone may be and
/// still hold a vertex.
const REACH: f32 = 3.0;

/// Weights every vertex of `mesh` by its distance to `bones` — each a
/// segment, start to tip, in the mesh's own space in the bind pose. The
/// nearest [`MAX_INFLUENCES`] hold it. With no bones, every vertex is held
/// by none.
pub fn automatic(mesh: &mut SkinMesh, bones: &[(Vec2, Vec2)]) {
    for point in &mut mesh.vertices {
        let mut near: Vec<(u32, f32)> = bones
            .iter()
            .enumerate()
            .map(|(i, &(a, b))| (i as u32, distance_to_segment(point.position, a, b)))
            .filter(|(_, d)| d.is_finite())
            .collect();
        near.sort_by(|a, b| a.1.total_cmp(&b.1));
        // A bone three times as far as the nearest holds nothing: a joint
        // is shared by the bones that meet there, and a shoulder does not
        // follow the hand.
        let nearest = near.first().map_or(0.0, |(_, d)| *d);
        near.retain(|(_, d)| *d <= nearest * REACH);
        near.truncate(MAX_INFLUENCES);
        point.weights = near
            .into_iter()
            .map(|(bone, d)| (bone, 1.0 / (d * d + 1e-6)))
            .collect();
        normalize(&mut point.weights);
    }
}

/// One dab of the brush: within `radius` of `centre` (in the mesh's own
/// space), `bone`'s weight on each vertex changes by `amount` — positive
/// adds, negative takes away — falling off smoothly to nothing at the edge.
/// Returns how many vertices it touched.
pub fn paint(mesh: &mut SkinMesh, centre: Vec2, radius: f32, bone: u32, amount: f32) -> usize {
    if !(radius > 0.0 && radius.is_finite() && amount.is_finite()) {
        return 0;
    }
    let mut touched = 0;
    for index in 0..mesh.vertices.len() {
        let d = mesh.vertices[index].position.distance(centre);
        if d >= radius {
            continue;
        }
        let t = 1.0 - d / radius;
        if paint_vertex(mesh, index, bone, amount * t * t * (3.0 - 2.0 * t)) {
            touched += 1;
        }
    }
    touched
}

/// Changes `bone`'s weight on vertex `index` by `amount`, renormalises it
/// and keeps its four strongest. Returns whether anything changed: taking
/// away a bone that does not hold it changes nothing.
pub fn paint_vertex(mesh: &mut SkinMesh, index: usize, bone: u32, amount: f32) -> bool {
    let Some(point) = mesh.vertices.get_mut(index) else {
        return false;
    };
    if !amount.is_finite() {
        return false;
    }
    match point.weights.iter_mut().find(|(b, _)| *b == bone) {
        Some((_, weight)) => *weight = (*weight + amount).max(0.0),
        None if amount > 0.0 => point.weights.push((bone, amount)),
        None => return false,
    }
    point.weights.retain(|(_, w)| *w > 1e-4);
    point
        .weights
        .sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    point.weights.truncate(MAX_INFLUENCES);
    normalize(&mut point.weights);
    true
}

/// How much `bone` holds each vertex, `0..=1`: what the editor colours
/// them by.
pub fn of_bone(mesh: &SkinMesh, bone: u32) -> Vec<f32> {
    mesh.vertices
        .iter()
        .map(|p| {
            p.weights
                .iter()
                .find(|(b, _)| *b == bone)
                .map_or(0.0, |(_, w)| *w)
        })
        .collect()
}

/// Scales `weights` to sum to one; leaves them empty if they sum to
/// nothing.
fn normalize(weights: &mut Vec<(u32, f32)>) {
    let total: f32 = weights.iter().map(|(_, w)| *w).sum();
    if total > 0.0 && total.is_finite() {
        for (_, w) in weights.iter_mut() {
            *w /= total;
        }
    } else {
        weights.clear();
    }
}

fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = if ab.length_squared() > 0.0 {
        ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.distance(a + ab * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skinned::SkinPoint;

    fn strip() -> SkinMesh {
        SkinMesh {
            vertices: (0..=4)
                .map(|i| SkinPoint {
                    position: Vec2::new(0.0, -(i as f32) * 0.5),
                    uv: Vec2::ZERO,
                    weights: Vec::new(),
                })
                .collect(),
            triangles: Vec::new(),
        }
    }

    /// An upper bone down to y = -1, a lower one on to y = -2.
    fn arm() -> [(Vec2, Vec2); 2] {
        [
            (Vec2::ZERO, Vec2::new(0.0, -1.0)),
            (Vec2::new(0.0, -1.0), Vec2::new(0.0, -2.0)),
        ]
    }

    #[test]
    fn automatic_weights_follow_the_nearest_bone_and_share_the_joint() {
        let mut mesh = strip();
        automatic(&mut mesh, &arm());
        let upper = of_bone(&mesh, 0);
        assert!(upper[0] > 0.99, "the shoulder is the upper bone's");
        assert!(upper[4] < 0.01, "the wrist is the lower bone's");
        assert!(
            (upper[2] - 0.5).abs() < 0.01,
            "the elbow is shared: {}",
            upper[2]
        );
        for point in &mesh.vertices {
            let total: f32 = point.weights.iter().map(|(_, w)| w).sum();
            assert!((total - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn the_brush_adds_and_takes_away_softly_and_keeps_four() {
        let mut mesh = strip();
        automatic(&mut mesh, &arm());
        let before = of_bone(&mesh, 1)[1];
        assert_eq!(paint(&mut mesh, Vec2::new(0.0, -0.5), 0.6, 1, 0.5), 3);
        let after = of_bone(&mesh, 1);
        assert!(after[1] > before, "{before} then {}", after[1]);
        assert!(of_bone(&mesh, 1)[4] > 0.99, "out of reach, unchanged");
        paint(&mut mesh, Vec2::new(0.0, -0.5), 0.6, 1, -10.0);
        assert_eq!(of_bone(&mesh, 1)[1], 0.0, "taken away entirely");
        for bone in 2..8 {
            paint(&mut mesh, Vec2::ZERO, 1.0, bone, 1.0);
        }
        assert!(mesh.vertices[0].weights.len() <= MAX_INFLUENCES);
    }

    #[test]
    fn a_brush_with_no_size_does_nothing() {
        let mut mesh = strip();
        assert_eq!(paint(&mut mesh, Vec2::ZERO, 0.0, 0, 1.0), 0);
        assert_eq!(paint(&mut mesh, Vec2::ZERO, f32::NAN, 0, 1.0), 0);
    }
}
