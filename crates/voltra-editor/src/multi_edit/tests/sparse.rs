//! Values whose fields come and go with their contents: maps left out
//! when empty, and enums picked again.

use voltra_scene::format::serial_tree::Step;
use voltra_scene::format::ComponentRegistry;
use voltra_scene::tilemap::{TileData, TileShape};
use voltra_scene::{Joint2D, JointKind, Tilemap};

use super::super::Touch;
use super::{begin, three, RED};

fn sloped() -> TileData {
    TileData {
        shape: TileShape::Slope {
            left: 0.0,
            right: 1.0,
        },
        ..TileData::default()
    }
}

/// A layer holding tile data for `frames`, each sloped.
fn layer(frames: &[u32]) -> Tilemap {
    let mut map = Tilemap::default();
    for &frame in frames {
        map.tile_data.insert(frame, sloped());
    }
    map
}

fn frames(map: &Tilemap) -> Vec<u32> {
    map.tile_data.keys().copied().collect()
}

#[test]
fn a_tiles_data_emptied_on_the_primary_goes_from_the_rest_alone() {
    let registry = ComponentRegistry::with_defaults();
    let (mut world, selected) = three([RED; 3]);
    world.insert(selected[0], layer(&[5, 7]));
    world.insert(selected[1], layer(&[5, 7]));
    world.insert(selected[2], layer(&[5]));
    let edit = begin(&world, &registry, &selected);
    // Frame 5 set back to a full tile: plain data is not kept, and with
    // none left the map is not written.
    world
        .get_mut::<Tilemap>(selected[2])
        .expect("t")
        .tile_data
        .clear();
    edit.finish(&mut world, &registry);
    for &other in &selected[..2] {
        let map = world.get::<Tilemap>(other).expect("t");
        assert_eq!(frames(map), [7], "frame 7 kept");
        assert_eq!(map.tile_data.get(&7), Some(&sloped()));
    }
}

#[test]
fn a_first_tiles_data_on_the_primary_is_added_to_the_rest() {
    let registry = ComponentRegistry::with_defaults();
    let (mut world, selected) = three([RED; 3]);
    world.insert(selected[0], layer(&[7]));
    world.insert(selected[1], layer(&[]));
    world.insert(selected[2], layer(&[]));
    let edit = begin(&world, &registry, &selected);
    assert_eq!(
        edit.mixed("Tilemap"),
        [vec![
            Step::Field("tile_data"),
            Step::Key(voltra_scene::format::SerialTree::U32(7))
        ]]
    );
    world
        .get_mut::<Tilemap>(selected[2])
        .expect("t")
        .tile_data
        .insert(5, sloped());
    edit.finish(&mut world, &registry);
    assert_eq!(
        frames(world.get::<Tilemap>(selected[0]).expect("t")),
        [5, 7]
    );
    assert_eq!(frames(world.get::<Tilemap>(selected[1]).expect("t")), [5]);
}

#[test]
fn a_full_tile_picked_again_where_the_primary_has_no_data_resets_the_rests() {
    let registry = ComponentRegistry::with_defaults();
    let (mut world, selected) = three([RED; 3]);
    world.insert(selected[0], layer(&[5]));
    world.insert(selected[1], layer(&[5, 7]));
    world.insert(selected[2], layer(&[]));
    let mut edit = begin(&world, &registry, &selected);
    let shape = vec![
        Step::Field("tile_data"),
        Step::Key(voltra_scene::format::SerialTree::U32(5)),
        Step::Field("shape"),
    ];
    edit.touch("Tilemap", shape, Touch::Variant);
    edit.finish(&mut world, &registry);
    // Frame 5's shape is the default again on each; frame 7 untouched.
    for &other in &selected[..2] {
        let map = world.get::<Tilemap>(other).expect("t");
        assert_eq!(
            map.tile_data.get(&5).map(|d| d.shape.clone()),
            Some(TileShape::Full)
        );
    }
    let map = world.get::<Tilemap>(selected[1]).expect("t");
    assert_eq!(map.tile_data.get(&7), Some(&sloped()));
}

#[test]
fn a_kind_picked_again_turns_only_the_other_kinds() {
    let registry = ComponentRegistry::with_defaults();
    let (mut world, selected) = three([RED; 3]);
    let rod = |length| Joint2D {
        kind: JointKind::rod(length),
        ..Joint2D::default()
    };
    world.insert(selected[0], rod(4.0));
    world.insert(
        selected[1],
        Joint2D {
            kind: JointKind::named("weld"),
            ..Joint2D::default()
        },
    );
    world.insert(selected[2], rod(1.0));
    let mut edit = begin(&world, &registry, &selected);
    edit.touch("Joint2D", vec![Step::Field("kind")], Touch::Variant);
    edit.finish(&mut world, &registry);
    let kind = |i: usize| world.get::<Joint2D>(selected[i]).expect("j").kind;
    assert_eq!(kind(0), JointKind::rod(4.0), "a rod keeps its own length");
    assert_eq!(
        kind(1),
        JointKind::rod(1.0),
        "the weld becomes the primary's"
    );
}
