//! Which instance roots are instances in their own right is recorded, not
//! worked out from the records their parts name: a coin dropped into a coin
//! names the coin's own records, and a row cut from a selection holding a
//! coin holds it under the coin file's own id.

use voltra_render::glam::Vec2;
use voltra_testkit::scratch_root;

use super::variant::write_coin;
use super::*;
use crate::format::{from_scene_file, SceneFile};

const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];

fn color_of(world: &World, entity: Entity) -> [f32; 4] {
    world.get::<Sprite2D>(entity).expect("a sprite").color
}

fn coin_instance(world: &mut World, prefabs: &mut Prefabs, coin: &AssetPath) -> Entity {
    let handle = prefabs.load(coin);
    spawn(
        world,
        &registry(),
        prefabs.get(handle),
        Transform2D::default(),
    )
    .expect("spawns")
    .root()
    .expect("a root")
}

#[test]
fn a_coin_dropped_into_a_coin_follows_its_file_and_cannot_be_applied_into_it() {
    let root = scratch_root();
    let mut prefabs = Prefabs::new(&root);
    let coin = write_coin(&mut prefabs, RED, 1.0);
    let mut world = World::new();
    let outer = coin_instance(&mut world, &mut prefabs, &coin);
    let inner = coin_instance(&mut world, &mut prefabs, &coin);
    set_parent(&mut world, inner, outer).expect("a plain reparent");
    let entities = world.entity_count();
    assert_eq!(instance_root(&world, inner), Some(inner), "its own");

    write_coin(&mut prefabs, GREEN, 1.0);
    let changed = prefabs.reload(&coin);
    sync_sources(&mut world, &registry(), &mut prefabs, &changed);
    assert_eq!(world.entity_count(), entities, "nothing lost or doubled");
    assert_eq!(color_of(&world, outer), GREEN);
    assert_eq!(color_of(&world, inner), GREEN, "followed its own file");

    let before = std::fs::read(root.join(coin.as_str())).expect("there");
    let applied = apply(&mut world, &registry(), &mut prefabs, outer);
    assert!(
        matches!(&applied, Err(WriteError::Cycle { through, .. }) if *through == coin),
        "{applied:?}"
    );
    assert_eq!(
        std::fs::read(root.join(coin.as_str())).expect("there"),
        before,
        "nothing written"
    );
    assert_eq!(world.entity_count(), entities);
}

#[test]
fn a_coin_deleted_from_a_row_cut_around_it_leaves_the_row_s_instances() {
    let root = scratch_root();
    let mut prefabs = Prefabs::new(&root);
    // Create prefab on the coin, then on the row holding it: the row's
    // record for the coin is the coin's own id, which the coin file has.
    let mut world = World::new();
    let row_entity = named(&mut world, "row", Vec2::ZERO);
    let coin_entity = named(&mut world, "coin", Vec2::ZERO);
    world.insert(coin_entity, Sprite2D::new(RED));
    set_parent(&mut world, coin_entity, row_entity).expect("a plain reparent");
    let coin = path("coin.prefab.ron");
    write(&world, &registry(), &mut prefabs, coin_entity, &coin).expect("written");
    link(&mut world, coin_entity, coin.clone());
    let row = path("row.prefab.ron");
    write(&world, &registry(), &mut prefabs, row_entity, &row).expect("written");
    let mut scene = World::new();
    let handle = prefabs.load(&row);
    let placed = spawn(
        &mut scene,
        &registry(),
        prefabs.get(handle),
        Transform2D::default(),
    )
    .expect("spawns")
    .root()
    .expect("a root");
    assert_eq!(children_of(&scene, placed).len(), 1);

    // The row saved without its coin.
    let mut edited = World::new();
    let alone = named(&mut edited, "row", Vec2::ZERO);
    let row_id = *world.get::<SceneId>(row_entity).expect("an id");
    edited.insert(alone, row_id);
    let changed = write(&edited, &registry(), &mut prefabs, alone, &row).expect("written");
    sync_sources(&mut scene, &registry(), &mut prefabs, &changed);

    assert!(
        children_of(&scene, placed).is_empty(),
        "the coin went with it"
    );
}

#[test]
fn a_file_from_before_reads_its_nested_instance_as_a_part_and_keeps_it_so() {
    let scratch = scratch_root();
    let mut prefabs = Prefabs::new(&scratch);
    // A level from before stage 126: no link says whether it is its own.
    let row = "019fe500-0000-7000-8000-00000000b001";
    let nested = "019fe500-0000-7000-8000-00000000b002";
    let glow = "019fe500-0000-7000-8000-00000000b003";
    let text = format!(
        r#"(version: 1, entities: [
            (id: "{row}", components: {{"Name": "row",
              "PrefabInstance": (source: Path("row.prefab.ron")),
              "PrefabPart": (record: "019fe500-0000-7000-8000-00000000a001")}}),
            (id: "{nested}", components: {{"Name": "coin", "Parent": "{row}",
              "PrefabInstance": (source: Path("coin.prefab.ron")),
              "PrefabPart": (record: "019fe500-0000-7000-8000-00000000a002")}}),
            (id: "{glow}", components: {{"Name": "glow", "Parent": "{nested}",
              "PrefabPart": (record: "019fe500-0000-7000-8000-00000000a003")}}),
        ])"#
    );
    let file: SceneFile = ron::from_str(&text).expect("reads");
    let mut world = World::new();
    from_scene_file(&file, &registry(), &mut world).expect("loads");
    let by_name = |world: &World, name: &str| {
        world
            .query::<Name>()
            .find(|(_, n)| n.as_str() == name)
            .map(|(e, _)| e)
            .expect("there")
    };
    let (top, inner) = (by_name(&world, "row"), by_name(&world, "coin"));

    assert_eq!(instance_root(&world, inner), Some(top), "a part of the row");
    sync_all(&mut world, &registry(), &mut prefabs);
    assert!(world.get::<PrefabInstance>(top).is_some_and(|l| l.own));

    // Saved and read again: the nested one is still a part, the root its own.
    let saved = crate::format::to_scene_file(&world, &registry()).expect("saves");
    let text = ron::ser::to_string(&saved).expect("writes");
    assert_eq!(text.matches("own:true").count(), 1, "{text}");
    let mut again = World::new();
    from_scene_file(&saved, &registry(), &mut again).expect("loads");
    let inner = by_name(&again, "coin");
    assert_eq!(instance_root(&again, inner), Some(by_name(&again, "row")));
}
