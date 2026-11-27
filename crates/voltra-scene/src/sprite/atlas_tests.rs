use super::*;
use voltra_testkit::scratch_root;

#[test]
fn a_default_sprite_has_no_slicing_at_all() {
    let sprite = Sprite2D::default();

    assert!(sprite.atlas.is_none());
    assert!(sprite.atlas_handle.is_none());
    assert!(sprite.region.is_none());
    assert_eq!(sprite.frame, 0);
    assert!(!sprite.flip_x && !sprite.flip_y);
    assert!(sprite.pixels_per_unit.is_none());
}

#[test]
fn a_scene_written_before_atlases_existed_still_loads() {
    // The whole point of `#[serde(default)]` on every new field: a project
    // on disk must open unchanged and draw exactly as it did.
    let text = "(color:(1.0,1.0,1.0,1.0),sort_order:0)";

    let sprite: Sprite2D = ron::from_str(text).expect("old scene shape");

    assert_eq!(sprite, Sprite2D::default());
}

#[test]
fn the_slicing_round_trips_without_its_handle() {
    let sprite = Sprite2D {
        atlas: Some(AssetPath::new("sprites/coin.atlas.ron").expect("valid")),
        frame: 3,
        flip_x: true,
        pixels_per_unit: Some(16.0),
        region: Some(Frame::new(8, 0, 8, 16)),
        ..Default::default()
    };

    let text = ron::to_string(&sprite).expect("serialize");
    assert!(
        !text.contains("atlas_handle"),
        "handle leaked into RON: {text}"
    );
    let back: Sprite2D = ron::from_str(&text).expect("deserialize");
    assert_eq!(back, sprite);
    assert!(back.atlas_handle.is_none(), "handle must not deserialize");
}

#[test]
fn setting_an_atlas_resolves_a_handle_and_clearing_it_drops_both() {
    let root = scratch_root();
    std::fs::write(
        root.join("coin.atlas.ron"),
        "(version: 1, grid: Some((cell: (16, 16), columns: 4, rows: 1)))",
    )
    .expect("the fixture writes");
    let mut atlases = Atlases::new(&root);
    let mut sprite = Sprite2D::default();

    sprite.set_atlas(
        Some(AssetPath::new("coin.atlas.ron").expect("valid")),
        &mut atlases,
    );

    let handle = sprite.atlas_handle.expect("a handle was resolved");
    assert_eq!(atlases.get(handle).len(), 4);

    sprite.set_atlas(None, &mut atlases);

    assert!(sprite.atlas.is_none());
    assert!(sprite.atlas_handle.is_none());
}
