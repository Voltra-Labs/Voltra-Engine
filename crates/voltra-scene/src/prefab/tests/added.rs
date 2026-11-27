//! A prefab added inside an instance — dropped under a variant's root in
//! its mode, under a part of an instance in a scene, or made there from a
//! selection — is that instance's own, kept by every sync and following
//! its own file, never taken for parts of the outer one and despawned.

use voltra_render::glam::Vec2;
use voltra_testkit::scratch_root;

use super::variant::{edit_in_place, spawned, write_coin};
use super::*;

const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
const YELLOW: [f32; 4] = [1.0, 1.0, 0.0, 1.0];

/// A gem of `color`, one entity, written to `gem.prefab.ron` as an edit on
/// disk is, under the same record id every time.
fn write_gem(prefabs: &mut Prefabs, color: [f32; 4]) -> AssetPath {
    let mut world = World::new();
    let gem = named(&mut world, "gem", Vec2::ZERO);
    world.insert(
        gem,
        SceneId(
            "019fe500-0000-7000-8000-0000000000f1"
                .parse()
                .expect("a uuid"),
        ),
    );
    world.insert(gem, Sprite::new(color));
    let target = path("gem.prefab.ron");
    let file = fragment(&world, &registry(), gem).expect("cut");
    crate::format::write(&file, &prefabs.root().join(target.as_str())).expect("written");
    target
}

/// The gem under `parent`, wherever it is.
fn gem_under(world: &World, parent: Entity) -> Option<Entity> {
    crate::hierarchy::descendants(world, parent)
        .into_iter()
        .find(|&e| world.get::<Name>(e).is_some_and(|n| n.as_str() == "gem"))
}

fn color_of(world: &World, entity: Entity) -> [f32; 4] {
    world.get::<Sprite>(entity).expect("a sprite").color
}

fn len_of(prefabs: &mut Prefabs, file: &AssetPath) -> usize {
    let handle = prefabs.load(file);
    prefabs.get(handle).len()
}

#[test]
fn a_prefab_added_to_a_variant_in_its_mode_is_kept_and_follows_its_file() {
    let root = scratch_root();
    let mut prefabs = Prefabs::new(&root);
    let base = write_coin(&mut prefabs, RED, 1.0);
    let gem = write_gem(&mut prefabs, YELLOW);
    let target = path("coin Variant.prefab.ron");
    variant(&registry(), &mut prefabs, &base, &target).expect("written");
    let before = len_of(&mut prefabs, &target);

    edit_in_place(&root, &mut prefabs, &target, |world, top| {
        let mut own = Prefabs::new(&root);
        let dropped = spawned(world, &mut own, &gem);
        set_parent(world, dropped, top).expect("a plain reparent");
    });

    let handle = prefabs.load(&target);
    assert!(prefabs.get(handle).problems().is_empty());
    assert_eq!(
        len_of(&mut prefabs, &target),
        before + 1,
        "read with the gem"
    );
    let mut world = World::new();
    let coin = spawned(&mut world, &mut prefabs, &target);
    let held = gem_under(&world, coin).expect("an instance has it");
    assert_eq!(color_of(&world, held), YELLOW);

    // Opened and saved again, it is still there; and a gem edit reaches it.
    edit_in_place(&root, &mut prefabs, &target, |_, _| {});
    let text = std::fs::read_to_string(root.join(target.as_str())).expect("there");
    assert!(text.contains("\"gem\""), "{text}");
    write_gem(&mut prefabs, GREEN);
    let changed = prefabs.reload(&gem);
    assert_eq!(changed, [gem, target]);
    sync_sources(&mut world, &registry(), &mut prefabs, &changed);
    assert_eq!(color_of(&world, held), GREEN);
}

#[test]
fn a_prefab_dropped_under_a_scene_instance_survives_its_sync_and_an_apply() {
    let root = scratch_root();
    let mut prefabs = Prefabs::new(&root);
    let base = write_coin(&mut prefabs, RED, 1.0);
    let gem = write_gem(&mut prefabs, YELLOW);
    let target = path("coin Variant.prefab.ron");
    variant(&registry(), &mut prefabs, &base, &target).expect("written");
    let before = len_of(&mut prefabs, &target);
    let mut world = World::new();
    let coin = spawned(&mut world, &mut prefabs, &target);
    let other = spawned(&mut world, &mut prefabs, &target);
    let dropped = spawned(&mut world, &mut prefabs, &gem);
    set_parent(&mut world, dropped, coin).expect("a plain reparent");
    assert_eq!(instance_root(&world, dropped), Some(dropped));

    // The variant's instances synced, and the gem's.
    sync_all(&mut world, &registry(), &mut prefabs);
    assert!(world.is_alive(dropped), "not taken for a lost part");
    write_gem(&mut prefabs, GREEN);
    let changed = prefabs.reload(&gem);
    sync_sources(&mut world, &registry(), &mut prefabs, &changed);
    assert_eq!(color_of(&world, dropped), GREEN, "following its own file");

    apply(&mut world, &registry(), &mut prefabs, coin).expect("applied");
    assert!(world.is_alive(dropped));
    assert_eq!(
        len_of(&mut prefabs, &target),
        before + 1,
        "the variant has it"
    );
    let given = gem_under(&world, other).expect("the other instance got it");
    assert_eq!(color_of(&world, given), GREEN);
    // Stored linked: a gem edit reaches it through the variant.
    write_gem(&mut prefabs, RED);
    let changed = prefabs.reload(&gem);
    assert!(changed.contains(&target), "{changed:?}");
    sync_sources(&mut world, &registry(), &mut prefabs, &changed);
    assert_eq!(color_of(&world, given), RED);
}

#[test]
fn a_prefab_made_from_a_selection_inside_a_variant_stays_in_it() {
    let root = scratch_root();
    let mut prefabs = Prefabs::new(&root);
    let base = write_coin(&mut prefabs, RED, 1.0);
    let target = path("coin Variant.prefab.ron");
    variant(&registry(), &mut prefabs, &base, &target).expect("written");
    let before = len_of(&mut prefabs, &target);
    let sparkle = path("sparkle.prefab.ron");

    // In the variant's mode: an entity added under its root, made a prefab
    // of its own and linked, as Create prefab from selection does.
    let mut own = Prefabs::new(&root);
    edit_in_place(&root, &mut prefabs, &target, |world, top| {
        let made = named(world, "sparkle", Vec2::ZERO);
        world.insert(made, Sprite::new(YELLOW));
        set_parent(world, made, top).expect("a plain reparent");
        write(world, &registry(), &mut own, made, &sparkle).expect("written");
        link(world, made, sparkle.clone());
    });

    assert_eq!(len_of(&mut prefabs, &target), before + 1);
    let mut world = World::new();
    let coin = spawned(&mut world, &mut prefabs, &target);
    sync_all(&mut world, &registry(), &mut prefabs);
    assert_eq!(crate::hierarchy::descendants(&world, coin).len(), before);
}

#[test]
fn a_prefab_added_whose_file_is_gone_is_kept() {
    let root = scratch_root();
    let mut prefabs = Prefabs::new(&root);
    let base = write_coin(&mut prefabs, RED, 1.0);
    let gem = write_gem(&mut prefabs, YELLOW);
    let mut world = World::new();
    let coin = spawned(&mut world, &mut prefabs, &base);
    let dropped = spawned(&mut world, &mut prefabs, &gem);
    set_parent(&mut world, dropped, coin).expect("a plain reparent");
    std::fs::remove_file(root.join(gem.as_str())).expect("removed");

    let mut fresh = Prefabs::new(&root);
    sync_all(&mut world, &registry(), &mut fresh);
    assert!(world.is_alive(dropped), "kept with its file gone");
    write_coin(&mut fresh, GREEN, 1.0);
    let changed = fresh.reload(&base);
    sync_sources(&mut world, &registry(), &mut fresh, &changed);
    assert!(world.is_alive(dropped), "and through the coin's edit");
    assert_eq!(color_of(&world, coin), GREEN);
}

/// The scene as an undo puts it back: the world written before, read again.
fn snapshot(world: &World) -> crate::format::SceneFile {
    crate::format::to_scene_file(world, &registry()).expect("records")
}

fn restored(file: &crate::format::SceneFile) -> World {
    let mut world = World::new();
    crate::format::from_scene_file(file, &registry(), &mut world).expect("loads");
    world
}

fn gems(world: &World) -> usize {
    world
        .query::<Name>()
        .filter(|(_, n)| n.as_str() == "gem")
        .count()
}

#[test]
fn an_apply_undone_or_redone_leaves_one_copy_of_what_it_wrote() {
    let root = scratch_root();
    let mut prefabs = Prefabs::new(&root);
    let base = write_coin(&mut prefabs, RED, 1.0);
    let gem = write_gem(&mut prefabs, YELLOW);
    let mut world = World::new();
    let coin = spawned(&mut world, &mut prefabs, &base);
    let dropped = spawned(&mut world, &mut prefabs, &gem);
    set_parent(&mut world, dropped, coin).expect("a plain reparent");
    let before = snapshot(&world);

    apply(&mut world, &registry(), &mut prefabs, coin).expect("applied");
    assert_eq!(gems(&world), 1);
    let after = snapshot(&world);

    // Undone: the scene as it was, the file still holding the gem.
    let mut undone = restored(&before);
    sync_all(&mut undone, &registry(), &mut prefabs);
    assert_eq!(gems(&undone), 1, "undone: not a second gem beside it");
    sync_all(&mut undone, &registry(), &mut prefabs);
    assert_eq!(gems(&undone), 1, "and still one on the next sync");

    // Redone: the scene as the Apply left it.
    let mut redone = restored(&after);
    sync_all(&mut redone, &registry(), &mut prefabs);
    assert_eq!(gems(&redone), 1, "redone");

    // A fresh instance of the coin has it once.
    let mut fresh = World::new();
    spawned(&mut fresh, &mut prefabs, &base);
    assert_eq!(gems(&fresh), 1);

    // Applied again from the undone scene: the same records, still one.
    let top = undone
        .query::<PrefabInstance>()
        .find(|(_, l)| l.source == base)
        .map(|(e, _)| e)
        .expect("the coin");
    apply(&mut undone, &registry(), &mut prefabs, top).expect("applied again");
    assert_eq!(gems(&undone), 1);
    let handle = prefabs.load(&base);
    assert_eq!(prefabs.get(handle).len(), 3, "coin, glow, one gem");
}
