//! The draw list over materials: grouped by pipeline and material, the
//! blended after the rest and back to front, a material that cannot draw
//! as the default.

use voltra_assets::{AssetPath, BuiltinMesh, Materials, Meshes};
use voltra_ecs::{Entity, World};
use voltra_render::glam::{Affine3A, Vec3};
use voltra_render::render3d::View3D;

use super::DrawList3D;
use crate::{MeshInstance3D, SceneId, Transform3D};

fn view() -> View3D {
    View3D::perspective(
        Affine3A::IDENTITY,
        std::f32::consts::FRAC_PI_2,
        1.0,
        0.1,
        None,
    )
}

fn path(raw: &str) -> AssetPath {
    AssetPath::new(raw).expect("a path")
}

/// A store holding an opaque, a masked and a blended surface material and
/// a sprite material, each loaded.
fn materials() -> (voltra_testkit::Scratch, Materials) {
    let root = voltra_testkit::scratch_root();
    let write = |name: &str, text: &str| {
        let at = root.join(name);
        std::fs::create_dir_all(at.parent().expect("a folder")).expect("folders");
        std::fs::write(at, text).expect("written");
    };
    write(
        "m/red.material.ron",
        "(version: 1, domain: Surface, standard: (base_color: (1.0, 0.0, 0.0, 1.0)))",
    );
    write(
        "m/leaf.material.ron",
        "(version: 1, domain: Surface, standard: (alpha: Mask(0.5)))",
    );
    write(
        "m/glass.material.ron",
        "(version: 1, domain: Surface, standard: (alpha: Blend))",
    );
    write(
        "m/flash.wgsl",
        "fn fragment(in: Fragment) -> vec4<f32> { return sprite_color(in); }",
    );
    write(
        "m/flash.material.ron",
        r#"(version: 1, shader: "m/flash.wgsl")"#,
    );
    let mut materials = Materials::new(root.path());
    for name in ["red", "leaf", "glass", "flash"] {
        materials.load(&path(&format!("m/{name}.material.ron")));
    }
    (root, materials)
}

fn spawn(world: &mut World, mesh: BuiltinMesh, at: Vec3, material: Option<&str>) -> Entity {
    let e = world.spawn();
    world.insert(e, SceneId::new());
    world.insert(e, Transform3D::from_translation(at));
    let mut instance = MeshInstance3D::new(mesh);
    instance.material = material.map(path);
    world.insert(e, instance);
    e
}

/// Each instance's distance in front of the eye, in draw order.
fn depths(list: &DrawList3D) -> Vec<f32> {
    list.instances.iter().map(|i| -i.model[2][3]).collect()
}

#[test]
fn blended_instances_draw_after_the_rest_back_to_front_in_any_spawn_order() {
    let (_root, materials) = materials();
    let glass = Some("m/glass.material.ron");
    let placed = [
        (BuiltinMesh::Plane, Vec3::new(0.0, 0.0, -3.0), glass),
        (BuiltinMesh::Plane, Vec3::new(0.2, 0.0, -6.0), glass),
        (BuiltinMesh::Cube, Vec3::new(0.0, 0.0, -9.0), None),
        (BuiltinMesh::Cube, Vec3::new(0.0, 0.0, -4.5), glass),
    ];
    let mut lists = Vec::new();
    for reverse in [false, true] {
        let mut world = World::new();
        let mut order: Vec<_> = placed.to_vec();
        if reverse {
            order.reverse();
        }
        for (mesh, at, material) in order {
            spawn(&mut world, mesh, at, material);
        }
        let mut meshes = Meshes::new();
        let list = DrawList3D::from_world(&world, &view(), &mut meshes, Some(&materials), None);
        lists.push((depths(&list), list.draws.clone()));
    }
    let (first, draws) = &lists[0];
    // The opaque cube first, then the three blended farthest first — a
    // plane, a cube, a plane: three draws, not one per mesh.
    assert_eq!(first, &[9.0, 6.0, 4.5, 3.0], "{first:?}");
    assert_eq!(draws.len(), 4);
    assert_eq!(draws[0].material, None);
    assert!(draws[1..].iter().all(|d| d.material.is_some()));
    assert_eq!(draws[1].mesh, draws[3].mesh, "the planes, apart");
    assert_ne!(draws[1].mesh, draws[2].mesh);
    // The same picture whichever was spawned first: depth decides.
    assert_eq!(lists[0].0, lists[1].0);
}

#[test]
fn opaque_draws_group_by_material_and_masks_follow_them() {
    let (_root, materials) = materials();
    let mut world = World::new();
    spawn(
        &mut world,
        BuiltinMesh::Cube,
        Vec3::new(0.0, 0.0, -2.0),
        Some("m/leaf.material.ron"),
    );
    spawn(
        &mut world,
        BuiltinMesh::Cube,
        Vec3::new(1.0, 0.0, -5.0),
        Some("m/red.material.ron"),
    );
    spawn(
        &mut world,
        BuiltinMesh::Cube,
        Vec3::new(-1.0, 0.0, -3.0),
        None,
    );
    spawn(
        &mut world,
        BuiltinMesh::Cube,
        Vec3::new(0.0, 1.0, -4.0),
        Some("m/red.material.ron"),
    );
    let mut meshes = Meshes::new();
    let list = DrawList3D::from_world(&world, &view(), &mut meshes, Some(&materials), None);
    let red = materials.by_path_handle(&path("m/red.material.ron"));
    let leaf = materials.by_path_handle(&path("m/leaf.material.ron"));
    let shape: Vec<_> = list
        .draws
        .iter()
        .map(|d| (d.material, d.instances.len()))
        .collect();
    // The default and the red are opaque, one draw each, the red's two
    // cubes front to back; the masked leaf after them.
    assert_eq!(shape, [(None, 1), (red, 2), (leaf, 1)]);
    assert_eq!(depths(&list), [3.0, 4.0, 5.0, 2.0]);
}

#[test]
fn a_sprite_material_or_one_not_loaded_draws_as_the_default() {
    let (_root, materials) = materials();
    let mut world = World::new();
    let sprite = spawn(
        &mut world,
        BuiltinMesh::Cube,
        Vec3::new(0.0, 0.0, -3.0),
        Some("m/flash.material.ron"),
    );
    spawn(
        &mut world,
        BuiltinMesh::Cube,
        Vec3::new(0.0, 0.0, -5.0),
        Some("m/never_loaded.material.ron"),
    );
    let mut meshes = Meshes::new();
    let list = DrawList3D::from_world(&world, &view(), &mut meshes, Some(&materials), None);
    assert_eq!(list.draws.len(), 1, "both the default's");
    assert_eq!(list.draws[0].material, None);
    assert_eq!(list.instances.len(), 2);
    assert!(
        list.warned_materials.contains(&sprite),
        "the sprite's said once"
    );
    assert_eq!(
        list.warned_materials.len(),
        1,
        "one not loaded yet is not wrong"
    );
}

#[test]
fn opaque_draws_of_one_custom_shader_sit_together() {
    let root = voltra_testkit::scratch_root();
    let write = |name: &str, text: &str| {
        let at = root.join(name);
        std::fs::create_dir_all(at.parent().expect("a folder")).expect("folders");
        std::fs::write(at, text).expect("written");
    };
    write(
        "m/tint.wgsl",
        "fn surface(in: SurfaceInput) -> Surface {\n    var s = standard_surface(in);\n    s.base_color = material.tint.rgb;\n    return s;\n}\n",
    );
    for (name, tint) in [("a", "1.0, 0.0, 0.0"), ("b", "0.0, 0.0, 1.0")] {
        write(
            &format!("m/tint_{name}.material.ron"),
            &format!(
                r#"(version: 1, domain: Surface, shader: "m/tint.wgsl", uniforms: {{"tint": Color(({tint}, 1.0))}})"#
            ),
        );
    }
    write("m/red.material.ron", "(version: 1, domain: Surface)");
    // Loaded so the standard red's slot falls between the two tints'.
    let mut materials = Materials::new(root.path());
    for name in ["tint_a", "red", "tint_b"] {
        materials.load(&path(&format!("m/{name}.material.ron")));
    }
    let mut world = World::new();
    for (x, name) in [(-1.0, "tint_a"), (0.0, "red"), (1.0, "tint_b")] {
        let material = format!("m/{name}.material.ron");
        spawn(
            &mut world,
            BuiltinMesh::Cube,
            Vec3::new(x, 0.0, -4.0),
            Some(material.as_str()),
        );
    }
    let mut meshes = Meshes::new();
    let list = DrawList3D::from_world(&world, &view(), &mut meshes, Some(&materials), None);
    let handle = |name: &str| materials.by_path_handle(&path(&format!("m/{name}.material.ron")));
    let order: Vec<_> = list.draws.iter().map(|d| d.material).collect();
    // The standard's first, then both of the one custom shader: one
    // pipeline switch, not two.
    assert_eq!(order, [handle("red"), handle("tint_a"), handle("tint_b")]);
}
