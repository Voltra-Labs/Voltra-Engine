//! An instance kept in step with its file: what a sync changes, what it
//! keeps, and what an instance saved before parts becomes.

use voltra_render::glam::Vec2;

use super::*;
use crate::format::{to_scene_file, SceneFile};
use crate::hierarchy::{children_of, set_parent};
use crate::name::Name;
use crate::prefab::spawn;
use crate::prefab::{sync_source, Prefabs};
use crate::{PointLight2D, Sprite, Transform};
use voltra_assets::AssetPath;

fn registry() -> ComponentRegistry {
    ComponentRegistry::with_defaults()
}

fn source() -> AssetPath {
    AssetPath::new("coin.ron").expect("valid")
}

fn named(world: &mut World, name: &str, at: Vec2) -> Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, Name::new(name));
    world.insert(entity, Transform::from_translation(at));
    entity
}

/// The authoring world of a coin: a red root with a light, and a glow
/// under it.
struct Authoring {
    world: World,
    coin: Entity,
    glow: Entity,
}

fn authoring() -> Authoring {
    let mut world = World::new();
    let coin = named(&mut world, "coin", Vec2::ZERO);
    world.insert(coin, Sprite::new([1.0, 0.0, 0.0, 1.0]));
    world.insert(coin, PointLight2D::default());
    let glow = named(&mut world, "glow", Vec2::new(0.0, 0.2));
    world.insert(glow, Sprite::default().with_sort_order(1));
    set_parent(&mut world, glow, coin).expect("a plain reparent");
    Authoring { world, coin, glow }
}

/// The prefab `world` saves as, read back through pretty-printed text as a
/// file on disk would be — so its values are not in a world's own spelling.
fn prefab_of(world: &World) -> Prefab {
    let file = to_scene_file(world, &registry()).expect("saves");
    let text =
        ron::ser::to_string_pretty(&file, ron::ser::PrettyConfig::default()).expect("writes");
    let file: SceneFile = ron::from_str(&text).expect("reads");
    Prefab::new(file)
        .expect("this build's version")
        .with_source(source())
}

fn instance(world: &mut World, prefab: &Prefab, at: Vec2) -> Entity {
    spawn(world, &registry(), prefab, &Transform::from_translation(at))
        .expect("spawns")
        .root()
        .expect("one root")
}

fn child_named(world: &World, parent: Entity, name: &str) -> Option<Entity> {
    children_of(world, parent)
        .into_iter()
        .find(|&c| world.get::<Name>(c).is_some_and(|n| n.as_str() == name))
}

#[test]
fn a_fresh_instance_is_already_in_step() {
    let made = authoring();
    let prefab = prefab_of(&made.world);
    let mut scene = World::new();
    let root = instance(&mut scene, &prefab, Vec2::new(5.0, 0.0));
    let report = sync(&mut scene, &registry(), &prefab, root).expect("syncs");
    assert!(
        report.is_empty(),
        "the file's spaces are not a change: {report:?}"
    );
}

#[test]
fn an_edit_to_the_file_reaches_the_instance_but_not_its_placement() {
    let mut made = authoring();
    let prefab = prefab_of(&made.world);
    let mut scene = World::new();
    let root = instance(&mut scene, &prefab, Vec2::new(5.0, 0.0));

    // The file changes: the coin turns blue, the glow moves, a shine appears
    // under the coin, and the light goes.
    made.world
        .insert(made.coin, Sprite::new([0.0, 0.0, 1.0, 1.0]));
    made.world.remove::<PointLight2D>(made.coin);
    made.world
        .insert(made.glow, Transform::from_translation(Vec2::new(0.0, 0.4)));
    let shine = named(&mut made.world, "shine", Vec2::new(0.1, 0.1));
    set_parent(&mut made.world, shine, made.coin).expect("a plain reparent");
    let edited = prefab_of(&made.world);

    let report = sync(&mut scene, &registry(), &edited, root).expect("syncs");
    assert_eq!(report.spawned, 1);
    assert_eq!(report.changed, 3, "colour, light, glow's place: {report:?}");
    assert_eq!(
        scene.get::<Sprite>(root).expect("sprite").color,
        [0.0, 0.0, 1.0, 1.0]
    );
    assert!(scene.get::<PointLight2D>(root).is_none());
    assert_eq!(
        scene.get::<Transform>(root).expect("placed").translation,
        Vec2::new(5.0, 0.0),
        "the root stays where it was put"
    );
    let glow = child_named(&scene, root, "glow").expect("still there");
    assert_eq!(
        scene.get::<Transform>(glow).expect("t").translation,
        Vec2::new(0.0, 0.4)
    );
    let shine = child_named(&scene, root, "shine").expect("spawned under the coin");
    let part = scene.get::<PrefabPart>(shine).expect("a part");
    assert!(part.overridden.is_empty());
    assert!(
        sync(&mut scene, &registry(), &edited, root)
            .expect("syncs")
            .is_empty(),
        "a second sync has nothing to do"
    );
}

#[test]
fn what_the_instance_made_its_own_is_kept() {
    let mut made = authoring();
    let prefab = prefab_of(&made.world);
    let mut scene = World::new();
    let root = instance(&mut scene, &prefab, Vec2::ZERO);
    // The instance is green, and has a sensor the prefab does not.
    scene.insert(root, Sprite::new([0.0, 1.0, 0.0, 1.0]));
    scene.insert(root, crate::Sensor);
    let part = scene.get_mut::<PrefabPart>(root).expect("a part");
    part.overridden.insert("Sprite".to_owned());
    part.overridden.insert("Sensor".to_owned());

    made.world
        .insert(made.coin, Sprite::new([0.0, 0.0, 1.0, 1.0]));
    made.world.insert(
        made.coin,
        PointLight2D {
            radius: 9.0,
            ..PointLight2D::default()
        },
    );
    let edited = prefab_of(&made.world);
    let report = sync(&mut scene, &registry(), &edited, root).expect("syncs");
    assert_eq!(report.changed, 1, "the light only: {report:?}");
    assert_eq!(
        scene.get::<Sprite>(root).expect("s").color,
        [0.0, 1.0, 0.0, 1.0]
    );
    assert!(scene.get::<crate::Sensor>(root).is_some());
}

#[test]
fn a_part_the_file_lost_goes_and_one_the_instance_deleted_stays_gone() {
    let mut made = authoring();
    let prefab = prefab_of(&made.world);
    let mut scene = World::new();
    let root = instance(&mut scene, &prefab, Vec2::ZERO);
    let glow_record = scene
        .get::<PrefabPart>(child_named(&scene, root, "glow").expect("glow"))
        .expect("a part")
        .record;

    // Deleted from the instance and remembered as such: not spawned back.
    let glow = child_named(&scene, root, "glow").expect("glow");
    hierarchy::despawn_recursive(&mut scene, glow);
    scene
        .get_mut::<PrefabInstance>(root)
        .expect("linked")
        .removed
        .push(glow_record);
    let report = sync(&mut scene, &registry(), &prefab, root).expect("syncs");
    assert_eq!(report.spawned, 0);

    // Another instance still has it; the file drops it, and it goes.
    let other = instance(&mut scene, &prefab, Vec2::new(3.0, 0.0));
    hierarchy::despawn_recursive(&mut made.world, made.glow);
    let edited = prefab_of(&made.world);
    let report = sync(&mut scene, &registry(), &edited, other).expect("syncs");
    assert_eq!(report.despawned, 1);
    assert!(child_named(&scene, other, "glow").is_none());
}

#[test]
fn an_instance_saved_before_parts_is_adopted_without_a_change() {
    let made = authoring();
    let prefab = prefab_of(&made.world);
    let mut scene = World::new();
    let root = instance(&mut scene, &prefab, Vec2::new(2.0, 0.0));
    // As a scene from before this stage holds it: no parts, an edit or two.
    for entity in std::iter::once(root).chain(hierarchy::descendants(&scene, root)) {
        scene.remove::<PrefabPart>(entity);
    }
    scene.insert(root, Sprite::new([0.5, 0.5, 0.5, 1.0]));
    let extra = named(&mut scene, "mine", Vec2::ZERO);
    set_parent(&mut scene, extra, root).expect("a plain reparent");

    let report = sync(&mut scene, &registry(), &prefab, root).expect("syncs");
    assert_eq!(report.adopted, 2, "the coin and its glow");
    assert_eq!(
        (report.changed, report.spawned, report.despawned),
        (0, 0, 0)
    );
    let part = scene.get::<PrefabPart>(root).expect("adopted");
    assert!(
        part.overridden.contains("Sprite"),
        "its edit is now an override"
    );
    assert!(
        scene.get::<PrefabPart>(extra).is_none(),
        "its own entity stays its own"
    );
    assert_eq!(
        scene.get::<Sprite>(root).expect("s").color,
        [0.5, 0.5, 0.5, 1.0]
    );
}

#[test]
fn a_prefab_that_will_not_read_leaves_its_instances_alone() {
    let made = authoring();
    let prefab = prefab_of(&made.world);
    let mut scene = World::new();
    let root = instance(&mut scene, &prefab, Vec2::ZERO);
    let report = sync(&mut scene, &registry(), &Prefab::empty(), root).expect("syncs");
    assert!(report.is_empty());
    assert_eq!(hierarchy::descendants(&scene, root).len(), 1);
}

#[test]
fn sync_source_reaches_only_that_prefab_s_instances() {
    let root_dir = voltra_testkit::scratch_root();
    let mut made = authoring();
    let write = |world: &World| {
        let file = to_scene_file(world, &registry()).expect("saves");
        crate::format::write(&file, &root_dir.join("coin.ron")).expect("writes");
    };
    write(&made.world);
    let mut prefabs = Prefabs::new(&root_dir);
    let handle = prefabs.load(&source());
    let mut scene = World::new();
    let root = instance(&mut scene, &prefabs.get(handle).clone(), Vec2::ZERO);
    let stray = named(&mut scene, "stray", Vec2::ZERO);
    scene.insert(
        stray,
        PrefabInstance::new(AssetPath::new("other.ron").expect("valid")),
    );

    made.world
        .insert(made.coin, Sprite::new([0.0, 0.0, 1.0, 1.0]));
    write(&made.world);
    assert!(!prefabs.reload(&source()).is_empty());
    let report = sync_source(&mut scene, &registry(), &mut prefabs, &source());
    assert_eq!(report.changed, 1);
    assert_eq!(
        scene.get::<Sprite>(root).expect("s").color,
        [0.0, 0.0, 1.0, 1.0]
    );
    assert!(
        scene.get::<PrefabPart>(stray).is_none(),
        "another prefab's is not touched"
    );
}

#[test]
fn a_joint_inside_a_prefab_joins_the_instance_s_own_bodies_and_follows_edits() {
    use crate::{Joint2D, JointKind};
    let mut made = authoring();
    let coin_id = *made.world.get::<SceneId>(made.coin).expect("an id");
    made.world.insert(
        made.glow,
        Joint2D {
            b: Some(coin_id),
            kind: JointKind::rod(0.2),
            ..Joint2D::default()
        },
    );
    let prefab = prefab_of(&made.world);
    let mut scene = World::new();
    let root = instance(&mut scene, &prefab, Vec2::ZERO);
    let glow = child_named(&scene, root, "glow").expect("glow");
    let root_id = *scene.get::<SceneId>(root).expect("an id");
    assert_eq!(
        scene.get::<Joint2D>(glow).expect("joint").b,
        Some(root_id),
        "the copy's own coin, not the prefab's"
    );
    assert!(
        sync(&mut scene, &registry(), &prefab, root)
            .expect("syncs")
            .is_empty(),
        "compared in the file's ids, nothing differs"
    );

    made.world.insert(
        made.glow,
        Joint2D {
            b: Some(coin_id),
            kind: JointKind::rope(0.5),
            ..Joint2D::default()
        },
    );
    let edited = prefab_of(&made.world);
    let report = sync(&mut scene, &registry(), &edited, root).expect("syncs");
    assert_eq!(report.changed, 1);
    let joint = scene.get::<Joint2D>(glow).expect("joint");
    assert_eq!(joint.kind, JointKind::rope(0.5));
    assert_eq!(joint.b, Some(root_id), "still the instance's coin");
}
