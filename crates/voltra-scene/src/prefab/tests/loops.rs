//! Loops through variants and nesting: refused before anything is
//! written, and one made by hand on disk read with an error, as is a chain
//! past the depth limit, rather than recursed into.

use std::path::Path;

use voltra_testkit::scratch_root;

use super::variant::{spawned, write_coin};
use super::*;

const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];

#[test]
fn a_variant_of_itself_or_through_a_chain_back_is_refused() {
    let root = scratch_root();
    let mut prefabs = Prefabs::new(&root);
    let base = write_coin(&mut prefabs, RED, 1.0);
    let before = std::fs::read(root.join(base.as_str())).expect("written");

    let itself = variant(&registry(), &mut prefabs, &base, &base);
    assert!(
        matches!(itself, Err(WriteError::Cycle { .. })),
        "{itself:?}"
    );

    let blue = path("blue.prefab.ron");
    variant(&registry(), &mut prefabs, &base, &blue).expect("written");
    let back = variant(&registry(), &mut prefabs, &blue, &base);
    assert!(
        matches!(&back, Err(WriteError::Cycle { through, .. }) if *through == blue),
        "{back:?}"
    );
    assert_eq!(
        std::fs::read(root.join(base.as_str())).expect("still there"),
        before,
        "nothing written"
    );
}

#[test]
fn a_prefab_nesting_itself_through_a_variant_is_refused() {
    let root = scratch_root();
    let mut prefabs = Prefabs::new(&root);
    let base = write_coin(&mut prefabs, RED, 1.0);
    let blue = path("blue.prefab.ron");
    variant(&registry(), &mut prefabs, &base, &blue).expect("written");

    // The coin given an instance of its own variant, and written over itself.
    let mut world = World::new();
    let coin = spawned(&mut world, &mut prefabs, &base);
    let inner = spawned(&mut world, &mut prefabs, &blue);
    set_parent(&mut world, inner, coin).expect("a plain reparent");
    let written = write(&world, &registry(), &mut prefabs, coin, &base);

    assert!(
        matches!(&written, Err(WriteError::Cycle { through, .. }) if *through == blue),
        "{written:?}"
    );
}

/// A variant whose base is `base`, written by hand.
fn hand_written(root: &Path, file: &str, base: &str) {
    let text = format!(
        r#"(version: 1, entities: [
            (id: "019fe500-0000-7000-8000-0000000000d1", components: {{
                "Name": "loop",
                "PrefabInstance": (source: Path("{base}")),
                "PrefabPart": (record: "019fe500-0000-7000-8000-0000000000d1"),
            }}),
        ])"#
    );
    std::fs::write(root.join(file), text).expect("written");
}

#[test]
fn a_loop_written_on_disk_reads_with_an_error_instead_of_recursing() {
    let root = scratch_root();
    hand_written(&root, "self.prefab.ron", "self.prefab.ron");
    hand_written(&root, "a.prefab.ron", "b.prefab.ron");
    hand_written(&root, "b.prefab.ron", "a.prefab.ron");
    let mut prefabs = Prefabs::new(&root);

    let alone = prefabs.load(&path("self.prefab.ron"));
    let a = prefabs.load(&path("a.prefab.ron"));
    let b = prefabs.load(&path("b.prefab.ron"));

    assert!(matches!(
        prefabs.get(alone).problems(),
        [ResolveProblem::Cycle { chain }] if chain.len() == 2
    ));
    // `a` read `b` on the way, and `b` found `a` being read.
    assert!(prefabs.get(a).problems().is_empty());
    assert!(matches!(
        prefabs.get(b).problems(),
        [ResolveProblem::Cycle { chain }] if *chain == [path("a.prefab.ron"), path("b.prefab.ron"), path("a.prefab.ron")]
    ));
    // Each still spawns what its file holds, and a reload ends too.
    assert_eq!(prefabs.get(a).len(), 1);
    let text = std::fs::read_to_string(root.join("a.prefab.ron")).expect("there");
    std::fs::write(
        root.join("a.prefab.ron"),
        text.replace("\"loop\"", "\"looped\""),
    )
    .expect("written");
    assert_eq!(
        prefabs.reload(&path("a.prefab.ron")),
        [path("a.prefab.ron"), path("b.prefab.ron")]
    );
}

#[test]
fn a_chain_deeper_than_the_limit_stops_with_an_error() {
    let root = scratch_root();
    let name = |n: usize| format!("v{n}.prefab.ron");
    std::fs::write(
        root.join(name(0)),
        r#"(version: 1, entities: [(id: "019fe500-0000-7000-8000-0000000000e1", components: {"Name": "base"})])"#,
    )
    .expect("written");
    for n in 1..=MAX_DEPTH + 4 {
        hand_written(&root, &name(n), &name(n - 1));
    }
    let mut prefabs = Prefabs::new(&root);

    let top = prefabs.load(&path(&name(MAX_DEPTH + 4)));

    assert!(prefabs.get(top).problems().is_empty());
    // The top and the 31 below it are read; the last of them stops.
    let deepest = prefabs
        .by_path_handle(&path(&name(5)))
        .expect("read on the way down");
    assert!(matches!(
        prefabs.get(deepest).problems(),
        [ResolveProblem::TooDeep { source }] if *source == path(&name(4))
    ));
    assert!(prefabs.by_path_handle(&path(&name(4))).is_none());
}

#[test]
fn a_loop_through_a_nested_instance_on_disk_reads_with_an_error() {
    let root = scratch_root();
    // `a` holds, under a plain root, an instance of `b`; `b` is a variant
    // of `a`.
    std::fs::write(
        root.join("a.prefab.ron"),
        r#"(version: 1, entities: [
            (id: "019fe500-0000-7000-8000-0000000000a1", components: {"Name": "a"}),
            (id: "019fe500-0000-7000-8000-0000000000a2", components: {
                "Name": "inner",
                "Parent": "019fe500-0000-7000-8000-0000000000a1",
                "PrefabInstance": (source: Path("b.prefab.ron")),
                "PrefabPart": (record: "019fe500-0000-7000-8000-0000000000d1"),
            }),
        ])"#,
    )
    .expect("written");
    hand_written(&root, "b.prefab.ron", "a.prefab.ron");
    let mut prefabs = Prefabs::new(&root);

    let a = prefabs.load(&path("a.prefab.ron"));

    let b = prefabs
        .by_path_handle(&path("b.prefab.ron"))
        .expect("read on the way");
    assert!(matches!(
        prefabs.get(b).problems(),
        [ResolveProblem::Cycle { chain }] if chain.len() == 3
    ));
    assert_eq!(prefabs.get(a).len(), 2, "as its file has it");
    let mut world = World::new();
    spawn(
        &mut world,
        &registry(),
        prefabs.get(a),
        &Transform::default(),
    )
    .expect("spawns");
    assert_eq!(world.entity_count(), 2);
}
