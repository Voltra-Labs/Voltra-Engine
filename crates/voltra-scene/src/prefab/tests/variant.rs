//! Variants: a prefab whose root is an instance of another, with overrides
//! of its own — base edits reaching it and its instances but for what
//! either layer overrides, chains, and the loops refused or caught.

use std::path::Path;

use voltra_render::glam::Vec2;
use voltra_testkit::scratch_root;

use super::*;
use crate::format::{load as load_scene, save as save_scene};
use crate::PointLight2D;

const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
const BLUE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
const YELLOW: [f32; 4] = [1.0, 1.0, 0.0, 1.0];

/// Writes the base coin — a sprite of `color` and a light of `intensity`,
/// a glow under it — to `coin.prefab.ron`.
pub(super) fn write_coin(prefabs: &mut Prefabs, color: [f32; 4], intensity: f32) -> AssetPath {
    let mut world = World::new();
    let coin = named(&mut world, "coin", Vec2::ZERO);
    world.insert(coin, Sprite::new(color));
    world.insert(
        coin,
        PointLight2D {
            intensity,
            ..PointLight2D::default()
        },
    );
    let glow = named(&mut world, "glow", Vec2::new(0.0, 0.2));
    world.insert(glow, Sprite::default().with_sort_order(1));
    set_parent(&mut world, glow, coin).expect("a plain reparent");
    let target = path("coin.prefab.ron");
    // The same ids every write, as a prefab saved again in its mode keeps
    // its records'.
    let ids = [
        "019fe500-0000-7000-8000-0000000000c1",
        "019fe500-0000-7000-8000-0000000000c2",
    ];
    for (entity, id) in [coin, glow].into_iter().zip(ids) {
        let id = SceneId(id.parse().expect("a uuid"));
        world.insert(entity, id);
    }
    set_parent(&mut world, glow, coin).expect("parented under the fixed id");
    write(&world, &registry(), prefabs, coin, &target).expect("written");
    target
}

/// Opens `file` as prefab mode does, has `edit` change its root, records
/// what that overrides, and saves it — a variant edited in its mode.
pub(super) fn edit_in_place(
    root: &Path,
    prefabs: &mut Prefabs,
    file: &AssetPath,
    edit: impl FnOnce(&mut World, Entity),
) {
    let full = root.join(file.as_str());
    let mut world = World::new();
    load_scene(&full, &registry(), &mut world).expect("reads");
    sync_all(&mut world, &registry(), prefabs);
    let top = world
        .query::<SceneId>()
        .map(|(e, _)| e)
        .find(|&e| parent_of(&world, e).is_none())
        .expect("a root");
    edit(&mut world, top);
    for entity in std::iter::once(top).chain(crate::hierarchy::descendants(&world, top)) {
        refresh_overrides(&mut world, &registry(), prefabs, entity).expect("compared");
    }
    save_scene(&world, &registry(), &full).expect("saved");
    prefabs.reload(file);
}

fn color_of(world: &World, entity: Entity) -> [f32; 4] {
    world.get::<Sprite>(entity).expect("a sprite").color
}

fn intensity_of(world: &World, entity: Entity) -> f32 {
    world
        .get::<PointLight2D>(entity)
        .expect("a light")
        .intensity
}

pub(super) fn spawned(world: &mut World, prefabs: &mut Prefabs, file: &AssetPath) -> Entity {
    let handle = prefabs.load(file);
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
fn a_variant_names_its_base_and_spawns_what_the_base_does() {
    let root = scratch_root();
    let mut prefabs = Prefabs::new(&root);
    let base = write_coin(&mut prefabs, RED, 1.0);
    let target = path(&format!("{}.prefab.ron", variant_name(&base)));
    assert_eq!(target.as_str(), "coin Variant.prefab.ron");

    variant(&registry(), &mut prefabs, &base, &target).expect("written");

    let handle = prefabs.load(&target);
    assert_eq!(prefabs.get(handle).base(), Some(base.clone()));
    assert!(prefabs.get(handle).problems().is_empty());
    let mut world = World::new();
    let coin = spawned(&mut world, &mut prefabs, &target);
    assert_eq!(color_of(&world, coin), RED);
    assert_eq!(children_of(&world, coin).len(), 1);
    assert_eq!(
        world.get::<PrefabInstance>(coin).map(|l| l.source.clone()),
        Some(target),
        "an instance of the variant, not of the base"
    );
}

#[test]
fn a_base_edit_reaches_a_variant_and_an_instance_of_it() {
    let root = scratch_root();
    let mut prefabs = Prefabs::new(&root);
    let base = write_coin(&mut prefabs, RED, 1.0);
    let target = path("blue.prefab.ron");
    variant(&registry(), &mut prefabs, &base, &target).expect("written");
    let mut world = World::new();
    let coin = spawned(&mut world, &mut prefabs, &target);

    write_coin(&mut prefabs, GREEN, 3.0);
    let changed = prefabs.reload(&base);
    assert_eq!(changed, [base.clone(), target.clone()]);
    sync_sources(&mut world, &registry(), &mut prefabs, &changed);

    assert_eq!(color_of(&world, coin), GREEN);
    assert_eq!(intensity_of(&world, coin), 3.0);
}

#[test]
fn a_variant_override_is_kept_and_an_instance_override_wins_over_it() {
    let root = scratch_root();
    let mut prefabs = Prefabs::new(&root);
    let base = write_coin(&mut prefabs, RED, 1.0);
    let target = path("blue.prefab.ron");
    variant(&registry(), &mut prefabs, &base, &target).expect("written");
    edit_in_place(&root, &mut prefabs, &target, |world, coin| {
        world.insert(coin, Sprite::new(BLUE));
    });

    let mut world = World::new();
    let plain = spawned(&mut world, &mut prefabs, &target);
    let own = spawned(&mut world, &mut prefabs, &target);
    world.insert(own, Sprite::new(YELLOW));
    refresh_overrides(&mut world, &registry(), &mut prefabs, own).expect("compared");
    assert_eq!(color_of(&world, plain), BLUE, "the variant's override");

    // The base changes both the colour both layers override and the light
    // neither does.
    write_coin(&mut prefabs, GREEN, 3.0);
    let changed = prefabs.reload(&base);
    sync_sources(&mut world, &registry(), &mut prefabs, &changed);

    assert_eq!(color_of(&world, plain), BLUE, "the variant's override kept");
    assert_eq!(intensity_of(&world, plain), 3.0);
    assert_eq!(color_of(&world, own), YELLOW, "the instance's over both");
    assert_eq!(intensity_of(&world, own), 3.0);

    // The variant's override changed: the plain instance follows it.
    edit_in_place(&root, &mut prefabs, &target, |world, coin| {
        world.insert(coin, Sprite::new(RED));
    });
    sync_sources(&mut world, &registry(), &mut prefabs, &[target]);
    assert_eq!(color_of(&world, plain), RED);
    assert_eq!(color_of(&world, own), YELLOW);
}

#[test]
fn a_variant_of_a_variant_follows_both() {
    let root = scratch_root();
    let mut prefabs = Prefabs::new(&root);
    let base = write_coin(&mut prefabs, RED, 1.0);
    let blue = path("blue.prefab.ron");
    let bright = path("bright blue.prefab.ron");
    variant(&registry(), &mut prefabs, &base, &blue).expect("written");
    edit_in_place(&root, &mut prefabs, &blue, |world, coin| {
        world.insert(coin, Sprite::new(BLUE));
    });
    variant(&registry(), &mut prefabs, &blue, &bright).expect("written");
    edit_in_place(&root, &mut prefabs, &bright, |world, coin| {
        if let Some(light) = world.get_mut::<PointLight2D>(coin) {
            light.radius = 9.0;
        }
    });
    let mut world = World::new();
    let coin = spawned(&mut world, &mut prefabs, &bright);

    write_coin(&mut prefabs, GREEN, 3.0);
    let changed = prefabs.reload(&base);
    assert_eq!(changed, [base, blue, bright.clone()], "nearest first");
    sync_sources(&mut world, &registry(), &mut prefabs, &changed);

    assert_eq!(color_of(&world, coin), BLUE, "the middle one's override");
    let light = world.get::<PointLight2D>(coin).expect("a light");
    assert_eq!(light.radius, 9.0, "its own");
    assert_eq!(
        light.intensity, 1.0,
        "a component is overridden whole: the radius kept the light"
    );
    let handle = prefabs.load(&bright);
    assert_eq!(prefabs.get(handle).base(), Some(path("blue.prefab.ron")));
}

#[test]
fn apply_from_an_instance_of_a_variant_writes_the_variant_not_the_base() {
    let root = scratch_root();
    let mut prefabs = Prefabs::new(&root);
    let base = write_coin(&mut prefabs, RED, 1.0);
    let before = std::fs::read(root.join(base.as_str())).expect("written");
    let blue = path("blue.prefab.ron");
    variant(&registry(), &mut prefabs, &base, &blue).expect("written");
    let mut world = World::new();
    let coin = spawned(&mut world, &mut prefabs, &blue);
    let other = spawned(&mut world, &mut prefabs, &blue);
    world.insert(coin, Sprite::new(BLUE));
    refresh_overrides(&mut world, &registry(), &mut prefabs, coin).expect("compared");

    apply(&mut world, &registry(), &mut prefabs, coin).expect("applied");

    assert_eq!(
        std::fs::read(root.join(base.as_str())).expect("there"),
        before
    );
    assert_eq!(color_of(&world, other), BLUE, "the other instance followed");
    let handle = prefabs.load(&blue);
    assert_eq!(
        prefabs.get(handle).base(),
        Some(base.clone()),
        "still a variant"
    );
    // The blue is the variant's override now: a base edit to it stops there.
    write_coin(&mut prefabs, GREEN, 3.0);
    let changed = prefabs.reload(&base);
    sync_sources(&mut world, &registry(), &mut prefabs, &changed);
    assert_eq!(color_of(&world, other), BLUE);
    assert_eq!(intensity_of(&world, other), 3.0);
}
