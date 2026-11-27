//! Multi-editing instances of a prefab and of its variant: each takes the
//! field edited, keeps what its own layer overrides, records the edit as an
//! override against its own prefab, and a base edit still reaches what
//! neither overrides.

use voltra_scene::format::ComponentRegistry;
use voltra_scene::{prefab, Sprite};
use voltra_testkit::scratch_root;

use super::{begin, BLUE, GREEN};
use crate::prefab::fixtures::{
    base_and_variant, color, intensity, overridden, path, spawned, write_coin, BASE, VARIANT,
};

#[test]
fn a_base_instance_and_a_variant_instance_edited_together_keep_their_layers() {
    let root = scratch_root();
    let registry = ComponentRegistry::with_defaults();
    let mut prefabs = base_and_variant(&root, &registry);
    let (base, variant) = (path(BASE), path(VARIANT));

    let mut world = voltra_ecs::World::new();
    let plain = spawned(&mut world, &registry, &mut prefabs, &base);
    let varied = spawned(&mut world, &registry, &mut prefabs, &variant);
    assert_eq!(intensity(&world, varied), 3.0, "the variant's own light");
    assert!(
        overridden(&world, varied).is_empty(),
        "nothing of its own yet"
    );

    // Both selected, the variant's the primary: its colour is edited.
    let selected = [plain, varied];
    let edit = begin(&world, &registry, &selected);
    world.get_mut::<Sprite>(varied).expect("s").color = GREEN;
    edit.finish(&mut world, &registry);
    for &entity in &selected {
        prefab::refresh_overrides(&mut world, &registry, &mut prefabs, entity).expect("compared");
    }

    assert_eq!(color(&world, plain), GREEN, "written across");
    assert_eq!(intensity(&world, plain), 1.0, "the base's light kept");
    assert_eq!(intensity(&world, varied), 3.0, "the variant's light kept");
    assert_eq!(overridden(&world, plain), ["Sprite"]);
    assert_eq!(
        overridden(&world, varied),
        ["Sprite"],
        "against the variant: its light is the variant's, not an override"
    );

    // The base changes its colour and light: neither instance's colour
    // moves (both override it); the plain one's light follows the base,
    // the variant's keeps the variant's.
    write_coin(&registry, &mut prefabs, BLUE, 2.0);
    prefab::sync_all(&mut world, &registry, &mut prefabs);
    assert_eq!(color(&world, plain), GREEN);
    assert_eq!(color(&world, varied), GREEN);
    assert_eq!(intensity(&world, plain), 2.0, "the base's edit reaches it");
    assert_eq!(intensity(&world, varied), 3.0, "still the variant's");
}
