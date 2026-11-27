//! A prefab nested in another: an edit to the inner file reaching every
//! instance of the outer one without the outer being saved again, the
//! outer's overrides on it kept, and a file from before still reading.

use voltra_render::glam::Vec2;
use voltra_testkit::scratch_root;

use super::variant::{edit_in_place, write_coin};
use super::*;
use crate::format::SceneFile;

const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
const BLUE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

/// A row: a plain root with two coins under it, the second overriding its
/// colour to blue, written to `row.prefab.ron`.
fn write_row(prefabs: &mut Prefabs) -> AssetPath {
    let coin = path("coin.prefab.ron");
    let handle = prefabs.load(&coin);
    let mut world = World::new();
    let row = named(&mut world, "row", Vec2::ZERO);
    for (n, x) in [-1.0, 1.0].into_iter().enumerate() {
        let at = Transform::from_translation(Vec2::new(x, 0.0));
        let one = spawn(&mut world, &registry(), prefabs.get(handle), &at)
            .expect("spawns")
            .root()
            .expect("a root");
        set_parent(&mut world, one, row).expect("a plain reparent");
        if n == 1 {
            world.insert(one, Sprite::new(BLUE));
            refresh_overrides(&mut world, &registry(), prefabs, one).expect("compared");
        }
    }
    let target = path("row.prefab.ron");
    write(&world, &registry(), prefabs, row, &target).expect("written");
    target
}

/// The coin roots under the row instance at `row`, in order.
fn coins(world: &World, row: Entity) -> Vec<Entity> {
    let mut coins = children_of(world, row);
    coins.sort_by(|a, b| {
        let x = |e: &Entity| world.get::<Transform>(*e).map_or(0.0, |t| t.translation.x);
        x(a).total_cmp(&x(b))
    });
    coins
}

fn color_of(world: &World, entity: Entity) -> [f32; 4] {
    world.get::<Sprite>(entity).expect("a sprite").color
}

fn row_instance(world: &mut World, prefabs: &mut Prefabs, row: &AssetPath) -> Entity {
    let handle = prefabs.load(row);
    spawn(
        world,
        &registry(),
        prefabs.get(handle),
        &Transform::default(),
    )
    .expect("spawns")
    .root()
    .expect("a root")
}

#[test]
fn an_inner_edit_reaches_instances_of_the_outer_without_saving_it() {
    let root = scratch_root();
    let mut prefabs = Prefabs::new(&root);
    let coin = write_coin(&mut prefabs, RED, 1.0);
    let row = write_row(&mut prefabs);
    let saved = std::fs::read(root.join(row.as_str())).expect("written");
    let mut world = World::new();
    let placed = row_instance(&mut world, &mut prefabs, &row);

    write_coin(&mut prefabs, GREEN, 1.0);
    let changed = prefabs.reload(&coin);
    assert_eq!(changed, [coin, row.clone()]);
    sync_sources(&mut world, &registry(), &mut prefabs, &changed);

    let [plain, blue] = coins(&world, placed)[..] else {
        panic!("two coins");
    };
    assert_eq!(color_of(&world, plain), GREEN, "followed the inner file");
    assert_eq!(color_of(&world, blue), BLUE, "the outer's override kept");
    assert_eq!(
        std::fs::read(root.join(row.as_str())).expect("there"),
        saved,
        "the outer file untouched"
    );
}

/// The coin again, given a spark under its glow.
fn write_coin_with_spark(prefabs: &mut Prefabs) {
    let coin = path("coin.prefab.ron");
    let mut world = World::new();
    let handle = prefabs.load(&coin);
    let file = prefabs.get(handle).file.clone().expect("the coin reads");
    crate::format::from_scene_file(&file, &registry(), &mut world).expect("loads");
    let top = world
        .query::<SceneId>()
        .map(|(e, _)| e)
        .find(|&e| parent_of(&world, e).is_none())
        .expect("a root");
    let glow = children_of(&world, top)[0];
    let spark = named(&mut world, "spark", Vec2::ZERO);
    set_parent(&mut world, spark, glow).expect("a plain reparent");
    write(&world, &registry(), prefabs, top, &coin).expect("written");
}

/// The ids of every record of `file` named `name`.
fn ids_named(file: &SceneFile, name: &str) -> Vec<SceneId> {
    let mut ids: Vec<SceneId> = file
        .entities
        .iter()
        .filter(|r| {
            r.components
                .get("Name")
                .and_then(|raw| raw.into_rust::<Name>().ok())
                .is_some_and(|n| n.as_str() == name)
        })
        .map(|r| r.id)
        .collect();
    ids.sort();
    ids
}

#[test]
fn a_record_the_inner_file_gains_reaches_the_outer_under_one_id_however_read() {
    let root = scratch_root();
    let mut prefabs = Prefabs::new(&root);
    let coin = write_coin(&mut prefabs, RED, 1.0);
    let row = write_row(&mut prefabs);
    let mut world = World::new();
    let placed = row_instance(&mut world, &mut prefabs, &row);

    write_coin_with_spark(&mut prefabs);
    let changed = prefabs.reload(&coin);
    sync_sources(&mut world, &registry(), &mut prefabs, &changed);

    for one in coins(&world, placed) {
        let glow = children_of(&world, one)[0];
        assert_eq!(children_of(&world, glow).len(), 1, "a spark under each");
    }
    let handle = prefabs.load(&row);
    let read = prefabs.get(handle).file.clone().expect("reads");
    let sparks = ids_named(&read, "spark");
    assert_eq!(sparks.len(), 2);

    // Read again, and opened as prefab mode opens it: the same ids, so an
    // instance of the row keeps naming its sparks.
    prefabs.reload(&row);
    let again = prefabs.get(handle).file.clone().expect("reads");
    assert_eq!(ids_named(&again, "spark"), sparks);
    let mut opened = World::new();
    crate::format::load(&root.join(row.as_str()), &registry(), &mut opened).expect("loads");
    sync_all(&mut opened, &registry(), &mut prefabs);
    let opened = to_scene_file(&opened, &registry()).expect("saves");
    assert_eq!(ids_named(&opened, "spark"), sparks);
}

#[test]
fn apply_on_an_instance_of_the_outer_keeps_its_inner_instances_linked() {
    let root = scratch_root();
    let mut prefabs = Prefabs::new(&root);
    let coin = write_coin(&mut prefabs, RED, 1.0);
    let row = write_row(&mut prefabs);
    let mut world = World::new();
    let placed = row_instance(&mut world, &mut prefabs, &row);
    world.insert(placed, Sprite::new(RED));
    refresh_overrides(&mut world, &registry(), &mut prefabs, placed).expect("compared");

    apply(&mut world, &registry(), &mut prefabs, placed).expect("applied");
    write_coin(&mut prefabs, GREEN, 1.0);
    let changed = prefabs.reload(&coin);
    sync_sources(&mut world, &registry(), &mut prefabs, &changed);

    let [plain, blue] = coins(&world, placed)[..] else {
        panic!("two coins");
    };
    assert_eq!(color_of(&world, placed), RED, "applied");
    assert_eq!(color_of(&world, plain), GREEN, "still following the coin");
    assert_eq!(color_of(&world, blue), BLUE, "still overriding it");
}

#[test]
fn an_outer_file_from_before_reads_as_it_was_and_follows_once_saved_again() {
    let root = scratch_root();
    let mut prefabs = Prefabs::new(&root);
    let coin = write_coin(&mut prefabs, RED, 1.0);
    let row = write_row(&mut prefabs);
    // As files were written before nested instances were linked: the inner
    // instance's records baked in, its root's link kept, no parts.
    let full = root.join(row.as_str());
    let text = std::fs::read_to_string(&full).expect("written");
    let mut file: SceneFile = ron::from_str(&text).expect("reads");
    for record in &mut file.entities {
        record.components.remove(PrefabPart::NAME);
    }
    crate::format::write(&file, &full).expect("written");
    prefabs.reload(&row);

    let handle = prefabs.load(&row);
    assert!(prefabs.get(handle).problems().is_empty());
    assert_eq!(prefabs.get(handle).len(), 5);
    let mut world = World::new();
    let placed = row_instance(&mut world, &mut prefabs, &row);
    let [plain, blue] = coins(&world, placed)[..] else {
        panic!("two coins");
    };
    assert_eq!(color_of(&world, plain), RED);
    assert_eq!(color_of(&world, blue), BLUE);

    // Saved again, as prefab mode saves it, its coins are linked.
    edit_in_place(&root, &mut prefabs, &row, |_, _| {});
    write_coin(&mut prefabs, GREEN, 1.0);
    let changed = prefabs.reload(&coin);
    sync_sources(&mut world, &registry(), &mut prefabs, &changed);
    assert_eq!(color_of(&world, plain), GREEN);
    assert_eq!(color_of(&world, blue), BLUE);
}
