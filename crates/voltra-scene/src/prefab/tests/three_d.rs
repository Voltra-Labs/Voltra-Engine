//! A 3D prefab through every place a root's placement is named: spawn,
//! sync, the override comparison, Apply, a fragment, a variant.

use voltra_render::glam::{Quat, Vec3};
use voltra_testkit::scratch_root;

use super::*;
use crate::transform3d::Transform3D;
use crate::PointLight2D;

/// Where the crate's file puts its root: off the origin, turned, scaled.
fn authored_root() -> Transform3D {
    Transform3D::from_translation(Vec3::new(0.0, 0.5, 0.0))
        .with_rotation(Quat::from_rotation_y(0.25))
        .with_scale(Vec3::splat(2.0))
}

fn named_3d(world: &mut World, name: &str, at: Transform3D) -> Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, Name::new(name));
    world.insert(entity, at);
    entity
}

/// A crate with a lamp one metre above it, written to `crate.prefab.ron`.
fn write_crate(prefabs: &Prefabs, intensity: f32) -> AssetPath {
    let mut world = World::new();
    let body = named_3d(&mut world, "crate", authored_root());
    let lamp = named_3d(
        &mut world,
        "lamp",
        Transform3D::from_translation(Vec3::new(0.0, 1.0, 0.0)),
    );
    // A component with no geometry in it, for an edit that is not the
    // placement.
    world.insert(
        lamp,
        PointLight2D {
            intensity,
            ..PointLight2D::default()
        },
    );
    for (entity, id) in [
        (body, "019fe500-0000-7000-8000-0000000003d1"),
        (lamp, "019fe500-0000-7000-8000-0000000003d2"),
    ] {
        world.insert(entity, SceneId(id.parse().expect("a uuid")));
    }
    set_parent(&mut world, lamp, body).expect("3D under 3D");
    let file = fragment(&world, &registry(), body).expect("cut");
    let target = path("crate.prefab.ron");
    crate::format::write(&file, &prefabs.root().join(target.as_str())).expect("written");
    target
}

fn spawned(world: &mut World, prefabs: &mut Prefabs, file: &AssetPath, at: Placement) -> Entity {
    let handle = prefabs.load(file);
    spawn(world, &registry(), prefabs.get(handle), at)
        .expect("spawns")
        .root()
        .expect("one root")
}

fn transform(world: &World, entity: Entity) -> Transform3D {
    *world.get::<Transform3D>(entity).expect("a 3D entity")
}

#[test]
fn a_3d_placement_places_the_root_and_the_children_keep_their_offsets() {
    let root = scratch_root();
    let mut prefabs = Prefabs::new(&root);
    let file = write_crate(&prefabs, 1.0);
    let at = Transform3D::from_translation(Vec3::new(10.0, 0.0, -4.0))
        .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2));
    let mut world = World::new();

    let body = spawned(&mut world, &mut prefabs, &file, at.into());

    let placed = transform(&world, body);
    let authored = authored_root();
    // The fragment zeroed the root's translation; the placement turned it
    // a quarter more and put it at `at`.
    assert!((placed.translation - at.translation).length() < 1e-5);
    let turned = at.rotation * authored.rotation;
    assert!(placed.rotation.abs_diff_eq(turned, 1e-6));
    assert_eq!(
        placed.scale, authored.scale,
        "an unscaled placement keeps it"
    );
    let lamp = children_of(&world, body)[0];
    assert_eq!(transform(&world, lamp).translation, Vec3::Y);
}

#[test]
fn a_placement_of_the_other_dimension_leaves_a_root_where_its_file_put_it() {
    let root = scratch_root();
    let mut prefabs = Prefabs::new(&root);
    let file = write_crate(&prefabs, 1.0);
    let mut world = World::new();

    let at = Transform2D::from_translation(Vec2::new(3.0, 4.0));
    let body = spawned(&mut world, &mut prefabs, &file, (&at).into());

    assert_eq!(transform(&world, body).translation, Vec3::ZERO);
    assert!(world.get::<Transform2D>(body).is_none(), "not given one");
}

#[test]
fn a_moved_3d_root_records_no_override_and_a_sync_leaves_it() {
    let root = scratch_root();
    let mut prefabs = Prefabs::new(&root);
    let file = write_crate(&prefabs, 1.0);
    let mut world = World::new();
    let body = spawned(&mut world, &mut prefabs, &file, Placement::AsAuthored);

    let moved = Transform3D::from_translation(Vec3::new(5.0, 0.0, 5.0))
        .with_rotation(Quat::from_rotation_x(1.0));
    world.insert(body, moved);
    refresh_overrides(&mut world, &registry(), &mut prefabs, body).expect("compared");

    assert!(
        world
            .get::<PrefabPart>(body)
            .expect("a part")
            .overridden
            .is_empty(),
        "the root's placement is never an override"
    );
    let report = sync_all(&mut world, &registry(), &mut prefabs);
    assert_eq!(report.changed, 0, "{report:?}");
    assert_eq!(transform(&world, body), moved);

    // A child's transform is no placement: moving one is an override.
    let lamp = children_of(&world, body)[0];
    world.insert(lamp, Transform3D::from_translation(Vec3::Z));
    refresh_overrides(&mut world, &registry(), &mut prefabs, lamp).expect("compared");
    assert!(world
        .get::<PrefabPart>(lamp)
        .expect("a part")
        .overridden
        .contains(Transform3D::NAME));
}

#[test]
fn apply_keeps_the_file_s_own_3d_root_transform() {
    let root = scratch_root();
    let mut prefabs = Prefabs::new(&root);
    let file = write_crate(&prefabs, 1.0);
    let mut world = World::new();
    let at = Transform3D::from_translation(Vec3::new(7.0, 0.0, 0.0));
    let body = spawned(&mut world, &mut prefabs, &file, at.into());
    let lamp = children_of(&world, body)[0];
    world.insert(
        lamp,
        PointLight2D {
            intensity: 4.0,
            ..PointLight2D::default()
        },
    );
    let stored_root = {
        let handle = prefabs.load(&file);
        prefabs.get(handle).file.as_ref().expect("reads").entities[0]
            .components
            .get(Transform3D::NAME)
            .expect("a 3D root")
            .get_ron()
            .to_owned()
    };

    apply(&mut world, &registry(), &mut prefabs, body).expect("applied");

    let handle = prefabs.load(&file);
    let written = prefabs.get(handle).file.as_ref().expect("reads");
    let top = &written.entities[0];
    assert_eq!(
        top.components
            .get(Transform3D::NAME)
            .expect("still 3D")
            .get_ron(),
        stored_root,
        "the world placement stays out of the file"
    );
    assert!(!top.components.contains_key(Transform2D::NAME));
    let lamp_record = written
        .entities
        .iter()
        .find(|r| r.components.contains_key(PointLight2D::NAME))
        .expect("the lamp");
    let light: PointLight2D = lamp_record.components[PointLight2D::NAME]
        .into_rust()
        .expect("reads");
    assert_eq!(light.intensity, 4.0, "the edit is written");
    assert_eq!(transform(&world, body).translation, at.translation);
}

#[test]
fn a_fragment_of_a_3d_selection_is_zeroed_at_its_root() {
    let mut world = World::new();
    let at = authored_root().with_rotation(Quat::from_rotation_z(0.5));
    let moved = Transform3D {
        translation: Vec3::new(4.0, -3.0, 9.0),
        ..at
    };
    let body = named_3d(&mut world, "crate", moved);
    let lamp = named_3d(&mut world, "lamp", Transform3D::from_translation(Vec3::Y));
    set_parent(&mut world, lamp, body).expect("3D under 3D");

    let file = fragment(&world, &registry(), body).expect("cut");

    let root: Transform3D = file.entities[0].components[Transform3D::NAME]
        .into_rust()
        .expect("reads");
    assert_eq!(root.translation, Vec3::ZERO);
    assert_eq!(root.rotation, moved.rotation);
    assert_eq!(root.scale, moved.scale);
    let child: Transform3D = file.entities[1].components[Transform3D::NAME]
        .into_rust()
        .expect("reads");
    assert_eq!(child.translation, Vec3::Y, "only the root is zeroed");
}

#[test]
fn a_variant_of_a_3d_prefab_spawns_as_its_base_and_follows_it() {
    let root = scratch_root();
    let mut prefabs = Prefabs::new(&root);
    let base = write_crate(&prefabs, 1.0);
    let target = path("crate Variant.prefab.ron");

    variant(&registry(), &mut prefabs, &base, &target).expect("written");

    let mut world = World::new();
    let body = spawned(&mut world, &mut prefabs, &target, Placement::AsAuthored);
    let base_root: Transform3D = {
        let handle = prefabs.load(&base);
        prefabs.get(handle).file.as_ref().expect("reads").entities[0].components[Transform3D::NAME]
            .into_rust()
            .expect("reads")
    };
    assert_eq!(transform(&world, body), base_root);
    let lamp = children_of(&world, body)[0];
    assert_eq!(transform(&world, lamp).translation, Vec3::Y);

    // A base edit reaches an instance of the variant.
    write_crate(&prefabs, 9.0);
    let changed = prefabs.reload(&base);
    sync_sources(&mut world, &registry(), &mut prefabs, &changed);
    let lamp = children_of(&world, body)[0];
    assert_eq!(
        world.get::<PointLight2D>(lamp).map(|l| l.intensity),
        Some(9.0)
    );
}

#[test]
fn apply_keeps_a_root_with_no_transform_in_its_file() {
    // 0.4's Apply skipped such a root's record entirely: the file lost it.
    let root = scratch_root();
    let mut prefabs = Prefabs::new(&root);
    let mut world = World::new();
    let rig = world.spawn();
    world.insert(rig, SceneId::new());
    world.insert(rig, Name::new("rig"));
    let lamp = named_3d(&mut world, "lamp", Transform3D::from_translation(Vec3::Y));
    set_parent(&mut world, lamp, rig).expect("3D under a bare group");
    let file = fragment(&world, &registry(), rig).expect("cut");
    let target = path("rig.prefab.ron");
    crate::format::write(&file, &prefabs.root().join(target.as_str())).expect("written");

    let mut scene = World::new();
    let top = spawned(&mut scene, &mut prefabs, &target, Placement::AsAuthored);
    scene.insert(top, Name::new("rig, renamed"));
    apply(&mut scene, &registry(), &mut prefabs, top).expect("applied");

    let handle = prefabs.load(&target);
    let written = prefabs.get(handle).file.as_ref().expect("reads");
    assert_eq!(written.entities.len(), 2, "the root is still in its file");
    let top_record = written
        .entities
        .iter()
        .find(|r| !r.components.contains_key(Parent::NAME))
        .expect("a root record");
    assert!(top_record.components.keys().all(|k| !Placement::names(k)));
    let name: Name = top_record.components[Name::NAME]
        .into_rust()
        .expect("reads");
    assert_eq!(name.as_str(), "rig, renamed");
}

#[test]
fn applying_a_root_that_changed_dimension_reports_the_other_instances() {
    let root = scratch_root();
    let mut prefabs = Prefabs::new(&root);
    let file = write_crate(&prefabs, 1.0);
    let mut world = World::new();
    let changed = spawned(&mut world, &mut prefabs, &file, Placement::AsAuthored);
    let other = spawned(&mut world, &mut prefabs, &file, Placement::AsAuthored);
    // The first instance's root made 2D (its child, too, to keep the chain
    // in one dimension).
    world.remove::<Transform3D>(changed);
    world.insert(changed, Transform2D::from_translation(Vec2::new(1.0, 1.0)));
    let lamp = children_of(&world, changed)[0];
    world.remove::<Transform3D>(lamp);
    world.insert(lamp, Transform2D::default());

    let report = apply(&mut world, &registry(), &mut prefabs, changed).expect("applied");

    let handle = prefabs.load(&file);
    let written = prefabs.get(handle).file.as_ref().expect("reads");
    let top = &written.entities[0];
    assert!(top.components.contains_key(Transform2D::NAME));
    assert!(
        !top.components.contains_key(Transform3D::NAME),
        "the old dimension is not kept beside the new one"
    );
    assert_eq!(report.other_dimension, vec![other]);
    assert_eq!(
        transform(&world, other),
        authored_root_at_origin(),
        "its placement is its own: left as it is"
    );
    let theirs = children_of(&world, other)[0];
    assert!(
        world.get::<Transform3D>(theirs).is_some() && world.get::<Transform2D>(theirs).is_none(),
        "its parts are not given the file's 2D transforms under a 3D root"
    );
    assert!(crate::hierarchy::dimension::problems(&world).is_empty());
}

/// The crate's root as its file stores it: the fragment zeroed it.
fn authored_root_at_origin() -> Transform3D {
    Transform3D {
        translation: Vec3::ZERO,
        ..authored_root()
    }
}
