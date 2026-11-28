//! Every built-in mesh: drawable, sized as documented, wound outward, its
//! normals, UVs and tangents agreeing with its triangles.

use std::f32::consts::PI;

use voltra_render::glam::{Vec2, Vec3};

use super::BuiltinMesh;

fn position(mesh: &voltra_render::render3d::MeshData3D, i: u32) -> Vec3 {
    Vec3::from(mesh.vertices[i as usize].position)
}

#[test]
fn every_builtin_is_drawable_and_sized_as_documented() {
    for (builtin, min, max) in [
        (BuiltinMesh::Cube, Vec3::splat(-0.5), Vec3::splat(0.5)),
        (BuiltinMesh::Sphere, Vec3::splat(-0.5), Vec3::splat(0.5)),
        (
            BuiltinMesh::Plane,
            Vec3::new(-0.5, 0.0, -0.5),
            Vec3::new(0.5, 0.0, 0.5),
        ),
        (
            BuiltinMesh::Capsule,
            Vec3::new(-0.5, -1.0, -0.5),
            Vec3::new(0.5, 1.0, 0.5),
        ),
        (BuiltinMesh::Cylinder, Vec3::splat(-0.5), Vec3::splat(0.5)),
    ] {
        let mesh = builtin.generate();
        assert_eq!(mesh.check(), Ok(()), "{builtin:?}");
        let bounds = mesh.bounds().expect("vertices");
        assert!(
            (bounds.min - min).abs().max_element() < 1e-5,
            "{builtin:?} {bounds:?}"
        );
        assert!(
            (bounds.max - max).abs().max_element() < 1e-5,
            "{builtin:?} {bounds:?}"
        );
        assert_eq!(BuiltinMesh::from_name(builtin.name()), Some(builtin));
    }
    assert_eq!(BuiltinMesh::from_name("torus"), None);
    // `Meshes` indexes its handles by discriminant.
    for (i, mesh) in BuiltinMesh::ALL.into_iter().enumerate() {
        assert_eq!(mesh as usize, i);
    }
}

#[test]
fn normals_and_tangents_are_unit_and_at_right_angles() {
    for builtin in BuiltinMesh::ALL {
        for (i, v) in builtin.generate().vertices.iter().enumerate() {
            let n = Vec3::from(v.normal);
            let t = Vec3::from_slice(&v.tangent[..3]);
            assert!(
                (n.length() - 1.0).abs() < 1e-4,
                "{builtin:?} {i}: normal {n}"
            );
            assert!(
                (t.length() - 1.0).abs() < 1e-4,
                "{builtin:?} {i}: tangent {t}"
            );
            assert!(n.dot(t).abs() < 1e-4, "{builtin:?} {i}: {n} · {t}");
            assert_eq!(v.tangent[3], 1.0);
            if builtin != BuiltinMesh::Plane {
                assert!(
                    n.dot(Vec3::from(v.position)) > 0.0,
                    "{builtin:?} {i}: inward"
                );
            }
        }
    }
}

#[test]
fn triangles_wind_outward_and_tangents_follow_the_uvs() {
    for builtin in BuiltinMesh::ALL {
        let mesh = builtin.generate();
        for tri in mesh.indices.as_chunks::<3>().0 {
            let [a, b, c] = tri.map(|i| position(&mesh, i));
            let face = (b - a).cross(c - a);
            assert!(
                face.length() > 1e-7,
                "{builtin:?}: a degenerate triangle {tri:?}"
            );
            let [va, vb, vc] = tri.map(|i| mesh.vertices[i as usize]);
            let normal = Vec3::from(va.normal) + Vec3::from(vb.normal) + Vec3::from(vc.normal);
            assert!(face.dot(normal) > 0.0, "{builtin:?}: {tri:?} winds inward");

            // dP/du and dP/dv from the triangle's UVs.
            let [ua, ub, uc] = [va.uv, vb.uv, vc.uv].map(Vec2::from);
            let (e1, e2) = (b - a, c - a);
            let (d1, d2) = (ub - ua, uc - ua);
            let det = d1.x * d2.y - d2.x * d1.y;
            if det.abs() < 1e-9 {
                continue;
            }
            let dp_du = (e1 * d2.y - e2 * d1.y) / det;
            let dp_dv = (e2 * d1.x - e1 * d2.x) / det;
            let n = Vec3::from(va.normal);
            let t = Vec3::from_slice(&va.tangent[..3]);
            assert!(
                t.dot(dp_du) > 0.0,
                "{builtin:?}: tangent against U at {tri:?}"
            );
            // V runs down the image; the bitangent points up it.
            assert!(
                n.cross(t).dot(dp_dv) < 0.0,
                "{builtin:?}: bitangent at {tri:?}"
            );
        }
    }
}

#[test]
fn closed_meshes_enclose_their_volume() {
    let r: f32 = 0.5;
    for (builtin, volume, tolerance) in [
        (BuiltinMesh::Cube, 1.0, 1e-5),
        (BuiltinMesh::Sphere, 4.0 / 3.0 * PI * r.powi(3), 0.03),
        (BuiltinMesh::Cylinder, PI * r * r, 0.01),
        (
            BuiltinMesh::Capsule,
            PI * r * r + 4.0 / 3.0 * PI * r.powi(3),
            0.03,
        ),
    ] {
        let mesh = builtin.generate();
        // The divergence theorem: only a closed, outward mesh sums to it.
        let enclosed: f32 = mesh
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| position(&mesh, i));
                a.dot(b.cross(c)) / 6.0
            })
            .sum();
        let error = (enclosed - volume).abs() / volume;
        assert!(error < tolerance, "{builtin:?}: {enclosed} for {volume}");
    }
}
