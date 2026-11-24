use super::*;

#[test]
fn a_grid_cuts_in_reading_order() {
    let grid = Grid::new(UVec2::new(16, 16), 3, 2);

    let frames = grid.frames();

    assert_eq!(frames.len(), 6);
    assert_eq!(frames[0], Frame::new(0, 0, 16, 16));
    assert_eq!(frames[2], Frame::new(32, 0, 16, 16));
    assert_eq!(frames[3], Frame::new(0, 16, 16, 16), "the second row");
    assert_eq!(grid.sheet(), UVec2::new(48, 32));
}

#[test]
fn a_margin_shifts_every_cell_and_a_padding_only_the_gaps() {
    let grid = Grid::new(UVec2::splat(8), 2, 1)
        .with_margin(UVec2::new(2, 3))
        .with_padding(UVec2::new(4, 0));

    let frames = grid.frames();

    assert_eq!(frames[0], Frame::new(2, 3, 8, 8));
    assert_eq!(frames[1], Frame::new(14, 3, 8, 8));
    assert_eq!(grid.sheet(), UVec2::new(2 * 2 + 16 + 4, 3 * 2 + 8));
}

#[test]
fn a_grid_with_no_size_cuts_nothing() {
    assert!(Grid::new(UVec2::ZERO, 4, 4).frames().is_empty());
    assert!(Grid::new(UVec2::splat(8), 0, 4).frames().is_empty());
    assert!(Grid::new(UVec2::splat(8), 4, 0).frames().is_empty());
    assert_eq!(Grid::new(UVec2::splat(8), 0, 4).sheet(), UVec2::ZERO);
}

#[test]
fn a_frame_becomes_the_uvs_it_covers() {
    let frame = Frame::new(16, 0, 16, 32);

    let (min, max) = frame.uv(UVec2::new(64, 32)).expect("it has area");

    assert_eq!(min, [0.25, 0.0]);
    assert_eq!(max, [0.5, 1.0]);
}

#[test]
fn an_empty_frame_or_sheet_has_no_uvs() {
    assert!(Frame::new(0, 0, 0, 8).uv(UVec2::splat(16)).is_none());
    assert!(Frame::new(0, 0, 8, 8).uv(UVec2::ZERO).is_none());
}

#[test]
fn a_frame_past_the_end_is_not_a_frame() {
    let atlas = Atlas::from_grid(Grid::new(UVec2::splat(8), 2, 1));

    assert_eq!(atlas.len(), 2);
    assert_eq!(atlas.frame(1), Some(Frame::new(8, 0, 8, 8)));
    assert_eq!(atlas.frame(2), None, "and not the last one again");
}

#[test]
fn a_file_of_a_grid_needs_no_sheet_of_its_own() {
    let file = AtlasFile {
        version: 1,
        sheet: None,
        texture: None,
        grid: Some(Grid::new(UVec2::splat(16), 4, 1)),
        frames: Vec::new(),
    };

    let atlas = file.into_atlas().expect("a grid is a slicing");

    assert_eq!(atlas.len(), 4);
    assert_eq!(atlas.sheet(), UVec2::new(64, 16));
}

#[test]
fn explicit_frames_are_appended_to_the_grid() {
    let file = AtlasFile {
        version: 1,
        sheet: Some(UVec2::new(64, 32)),
        texture: None,
        grid: Some(Grid::new(UVec2::splat(16), 2, 1)),
        frames: vec![Frame::new(0, 16, 64, 16)],
    };

    let atlas = file.into_atlas().expect("both forms in one file");

    assert_eq!(atlas.len(), 3);
    assert_eq!(atlas.frame(2), Some(Frame::new(0, 16, 64, 16)));
    assert_eq!(atlas.sheet(), UVec2::new(64, 32), "the explicit sheet wins");
}

#[test]
fn a_file_with_nothing_in_it_is_refused() {
    let file = AtlasFile {
        version: 1,
        sheet: Some(UVec2::splat(16)),
        texture: None,
        grid: None,
        frames: Vec::new(),
    };

    assert!(matches!(file.into_atlas(), Err(AtlasError::Empty)));
}

#[test]
fn explicit_frames_without_a_sheet_are_refused() {
    // Every UV would be a guess, and a guess that draws *something* is
    // worse than a refusal that says why.
    let file = AtlasFile {
        version: 1,
        sheet: None,
        texture: None,
        grid: None,
        frames: vec![Frame::new(0, 0, 8, 8)],
    };

    assert!(matches!(file.into_atlas(), Err(AtlasError::NoSheet)));
}

#[test]
fn a_wrong_version_is_refused() {
    let file = AtlasFile {
        version: 99,
        sheet: Some(UVec2::splat(16)),
        texture: None,
        grid: Some(Grid::new(UVec2::splat(16), 1, 1)),
        frames: Vec::new(),
    };

    assert!(matches!(file.into_atlas(), Err(AtlasError::Version(99))));
}

#[test]
fn a_file_round_trips_through_ron() {
    let text = r#"(
        version: 1,
        texture: Some("sprites/coin.png"),
        grid: Some((cell: (16, 16), columns: 4, rows: 1)),
    )"#;

    let file: AtlasFile = ron::from_str(text).expect("the fixture is valid RON");
    let atlas = file.into_atlas().expect("a grid slicing");

    assert_eq!(atlas.len(), 4);
    assert_eq!(
        atlas.texture().map(AssetPath::as_str),
        Some("sprites/coin.png")
    );
    assert_eq!(atlas.frame(3), Some(Frame::new(48, 0, 16, 16)));
}

#[test]
fn an_escaping_texture_hint_is_refused() {
    let file = AtlasFile {
        version: 1,
        sheet: None,
        texture: Some("../secrets.png".into()),
        grid: Some(Grid::new(UVec2::splat(8), 1, 1)),
        frames: Vec::new(),
    };

    assert!(matches!(file.into_atlas(), Err(AtlasError::Texture(_))));
}
