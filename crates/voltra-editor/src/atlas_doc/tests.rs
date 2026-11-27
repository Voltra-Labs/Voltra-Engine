use super::*;

fn root() -> voltra_testkit::Scratch {
    let dir = voltra_testkit::Scratch::new("voltra-atlas-doc");
    std::fs::create_dir_all(dir.join("sprites")).expect("scratch");
    dir
}

fn path(text: &str) -> AssetPath {
    AssetPath::new(text).expect("valid")
}

/// A 64×16 sheet with no atlas yet.
fn fresh(root: &std::path::Path) -> AtlasDoc {
    AtlasDoc::open(root, &path("sprites/coin.png"), |_| {
        Some(UVec2::new(64, 16))
    })
    .expect("a sheet opens")
}

#[test]
fn a_sheet_with_no_atlas_opens_empty_beside_it() {
    let root = root();
    let doc = fresh(&root);
    assert_eq!(doc.path().as_str(), "sprites/coin.atlas.ron");
    assert!(doc.regions().is_empty());
    assert!(!doc.is_dirty());
}

#[test]
fn a_grid_slicing_names_its_cells_after_the_sheet() {
    let root = root();
    let mut doc = fresh(&root);

    let cells = doc
        .slice_grid(UVec2::splat(16), UVec2::ZERO, UVec2::ZERO)
        .expect("slices");

    assert_eq!(cells, 4);
    assert_eq!(doc.regions()[2].name, "coin_2");
    assert_eq!(doc.regions()[2].frame, Frame::new(32, 0, 16, 16));
    assert!(doc.is_dirty());
}

#[test]
fn every_edit_is_one_undo_entry_and_redo_puts_it_back() {
    let root = root();
    let mut doc = fresh(&root);
    doc.slice_grid(UVec2::splat(16), UVec2::ZERO, UVec2::ZERO)
        .expect("slices");
    doc.rename(1, "spin").expect("renames");
    doc.set_pivot(1, Some(Vec2::new(0.5, 1.0))).expect("pivots");

    assert!(doc.undo());
    assert_eq!(doc.regions()[1].pivot, None);
    assert_eq!(doc.regions()[1].name, "spin");
    assert!(doc.undo());
    assert_eq!(doc.regions()[1].name, "coin_1");
    assert!(doc.undo());
    assert!(doc.regions().is_empty());
    assert!(!doc.undo(), "nothing before the opening");

    assert!(doc.redo());
    assert!(doc.redo());
    assert_eq!(doc.regions()[1].name, "spin");
}

#[test]
fn a_change_made_in_place_is_one_entry_once_settled() {
    let root = root();
    let mut doc = fresh(&root);
    doc.add(Frame::new(0, 0, 8, 8), Some("a")).expect("adds");

    // A field dragged over three frames.
    for width in [9, 10, 11] {
        doc.region_mut(0).expect("there").frame.width = width;
        doc.settle(false);
    }
    doc.settle(true);

    assert!(doc.undo());
    assert_eq!(
        doc.regions()[0].frame.width,
        8,
        "back before the drag, in one step"
    );
}

#[test]
fn a_duplicate_name_or_an_off_sheet_region_is_refused() {
    let root = root();
    let mut doc = fresh(&root);
    doc.add(Frame::new(0, 0, 8, 8), Some("a")).expect("adds");
    doc.add(Frame::new(8, 0, 8, 8), None).expect("adds");

    assert!(doc.rename(1, "a").is_err());
    assert!(doc.add(Frame::new(80, 0, 8, 8), None).is_err());
    doc.set_frame(0, Frame::new(60, 0, 8, 8))
        .expect("clamped onto the sheet");
    assert_eq!(doc.regions()[0].frame, Frame::new(60, 0, 4, 8));
    assert!(doc.rename(9, "b").is_err(), "no region 9");
}

#[test]
fn a_saved_atlas_opens_again_as_it_was() {
    let root = root();
    let mut doc = fresh(&root);
    doc.slice_grid(UVec2::splat(16), UVec2::ZERO, UVec2::ZERO)
        .expect("slices");
    doc.rename(3, "last").expect("renames");
    doc.set_pivot(0, Some(Vec2::new(0.0, 1.0))).expect("pivots");

    let file = doc.save(&root, None).expect("saves");
    assert_eq!(file, root.join("sprites/coin.atlas.ron"));
    assert!(!doc.is_dirty());

    let again = AtlasDoc::open(&root, &path("sprites/coin.atlas.ron"), |texture| {
        assert_eq!(texture.as_str(), "sprites/coin.png", "the texture hint");
        Some(UVec2::new(64, 16))
    })
    .expect("opens");
    assert_eq!(again.regions(), doc.regions());

    // The sheet opens its atlas too.
    let by_sheet = fresh(&root);
    assert_eq!(by_sheet.regions().len(), 4);
}

#[test]
fn an_empty_atlas_is_not_saved() {
    let root = root();
    let mut doc = fresh(&root);
    assert!(doc.save(&root, None).is_err());
    assert!(!root.join("sprites/coin.atlas.ron").exists());
}
