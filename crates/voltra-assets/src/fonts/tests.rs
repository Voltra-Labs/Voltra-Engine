use super::*;
use voltra_testkit::{copy_bundled_font, scratch_root, BUNDLED_FONT as UBUNTU};

fn path(raw: &str) -> AssetPath {
    AssetPath::new(raw).expect("a valid asset path")
}

/// A scratch root holding the default font where `Fonts::new` looks for it.
fn root_with_default() -> PathBuf {
    let root = scratch_root();
    copy_bundled_font(&root, DEFAULT_FONT);
    root
}

fn face(fonts: &Fonts, handle: Handle<Font>) -> &fontdue::Font {
    fonts.try_get(handle).and_then(Font::face).expect("a face")
}

#[test]
fn the_default_font_is_loaded_at_construction() {
    let fonts = Fonts::new(root_with_default());
    let font = fonts.try_get(fonts.default_font()).expect("resolves");
    assert!(font.face().is_some());
    assert_eq!(
        fonts.by_path_handle(&path(DEFAULT_FONT)),
        Some(fonts.default_font()),
        "naming the default by path shares its slot"
    );
}

#[test]
fn a_missing_default_is_a_faceless_slot_not_a_panic() {
    let fonts = Fonts::new(scratch_root());
    let font = fonts.try_get(fonts.default_font()).expect("still resolves");
    assert!(font.face().is_none());
    assert!(font.layout("hi", 32).is_none());
}

#[test]
fn one_path_is_read_once() {
    let root = root_with_default();
    std::fs::copy(UBUNTU, root.join("label.ttf")).expect("copies");
    let mut fonts = Fonts::new(&root);

    let first = fonts.load(&path("label.ttf"));
    let before = fonts.len();
    let second = fonts.load(&path("label.ttf"));

    assert_eq!(first, second);
    assert_eq!(fonts.len(), before);
}

#[test]
fn a_file_that_will_not_parse_draws_with_the_default_in_its_own_slot() {
    let root = root_with_default();
    std::fs::write(root.join("bad.ttf"), "not a font").expect("writes");
    let mut fonts = Fonts::new(&root);

    let handle = fonts.load(&path("bad.ttf"));

    assert_ne!(handle, fonts.default_font());
    assert!(fonts.try_get(handle).and_then(Font::face).is_some());
}

#[test]
fn a_broken_file_with_no_default_either_draws_nothing() {
    let root = scratch_root();
    std::fs::write(root.join("bad.ttf"), "not a font").expect("writes");
    let mut fonts = Fonts::new(&root);

    let handle = fonts.load(&path("bad.ttf"));

    assert!(fonts.try_get(handle).and_then(Font::face).is_none());
}

#[test]
fn a_reload_repairs_a_broken_file_under_the_same_handle() {
    let root = scratch_root();
    std::fs::write(root.join("label.ttf"), "half-written").expect("writes");
    let mut fonts = Fonts::new(&root);
    let handle = fonts.load(&path("label.ttf"));
    assert!(fonts.try_get(handle).and_then(Font::face).is_none());

    std::fs::copy(UBUNTU, root.join("label.ttf")).expect("copies");

    assert!(fonts.reload(&path("label.ttf")));
    assert!(fonts.try_get(handle).and_then(Font::face).is_some());
}

#[test]
fn a_reload_that_fails_keeps_the_face_and_its_glyphs() {
    let root = root_with_default();
    let mut fonts = Fonts::new(&root);
    let handle = fonts.default_font();
    fonts.prepare(handle, "A", 32);

    std::fs::write(root.join(DEFAULT_FONT), "truncated").expect("writes");

    assert!(!fonts.reload(&path(DEFAULT_FONT)));
    let font = fonts.try_get(handle).expect("resolves");
    assert!(font.face().is_some());
    assert_eq!(font.atlas().len(), 1);
}

#[test]
fn a_successful_reload_starts_the_atlas_over() {
    let root = root_with_default();
    let mut fonts = Fonts::new(&root);
    let handle = fonts.default_font();
    fonts.prepare(handle, "AB", 32);

    assert!(fonts.reload(&path(DEFAULT_FONT)));
    assert!(fonts.try_get(handle).expect("resolves").atlas().is_empty());
}

#[test]
fn reloading_a_path_nobody_loaded_does_nothing() {
    let root = root_with_default();
    std::fs::copy(UBUNTU, root.join("label.ttf")).expect("copies");
    let mut fonts = Fonts::new(&root);

    assert!(!fonts.reload(&path("label.ttf")));
    assert_eq!(fonts.len(), 1);
}

#[test]
fn prepare_rasterizes_each_glyph_once_per_size() {
    let mut fonts = Fonts::new(root_with_default());
    let handle = fonts.default_font();

    fonts.prepare(handle, "aab", 32);
    fonts.prepare(handle, "ab", 32);
    let atlas = fonts.try_get(handle).expect("resolves").atlas();
    assert_eq!(atlas.len(), 2);
    assert!(atlas.is_dirty());

    fonts.prepare(handle, "a", 48);
    assert_eq!(fonts.try_get(handle).expect("resolves").atlas().len(), 3);
}

#[test]
fn a_glyph_with_ink_gets_a_rect_and_a_space_does_not() {
    let mut fonts = Fonts::new(root_with_default());
    let handle = fonts.default_font();
    fonts.prepare(handle, "A ", 32);

    let font = fonts.try_get(handle).expect("resolves");
    let a = GlyphKey {
        glyph: face(&fonts, handle).lookup_glyph_index('A'),
        px: 32,
    };
    let space = GlyphKey {
        glyph: face(&fonts, handle).lookup_glyph_index(' '),
        px: 32,
    };
    assert!(font.atlas().get(a).and_then(|g| g.rect).is_some());
    assert_eq!(font.atlas().get(space).and_then(|g| g.rect), None);
}

mod layout {
    use super::*;

    fn laid(content: &str) -> TextLayout {
        let fonts = Fonts::new(root_with_default());
        fonts
            .try_get(fonts.default_font())
            .and_then(|font| font.layout(content, 32))
            .expect("a face")
    }

    #[test]
    fn pens_advance_left_to_right() {
        let out = laid("ab");
        assert_eq!(out.glyphs.len(), 2);
        assert_eq!(out.glyphs[0].pen, 0.0);
        assert!(out.glyphs[1].pen > 0.0);
        assert!(out.line_widths[0] > out.glyphs[1].pen);
    }

    #[test]
    fn a_newline_starts_a_second_line_at_the_left() {
        let out = laid("a\nb");
        assert_eq!(out.line_widths.len(), 2);
        assert_eq!(out.glyphs[1].line, 1);
        assert_eq!(out.glyphs[1].pen, 0.0);
        assert!(out.line_height > 0.0);
    }

    #[test]
    fn a_trailing_newline_is_an_empty_line() {
        let out = laid("a\n");
        assert_eq!(out.line_widths.len(), 2);
        assert_eq!(out.line_widths[1], 0.0);
    }

    #[test]
    fn carriage_returns_are_not_drawn() {
        assert_eq!(laid("a\r\nb").glyphs.len(), 2);
    }

    #[test]
    fn the_widest_line_is_the_width() {
        let out = laid("a\nabc");
        assert_eq!(out.width(), out.line_widths[1]);
    }

    #[test]
    fn ascent_is_above_the_baseline_and_descent_below() {
        let out = laid("g");
        assert!(out.ascent > 0.0);
        assert!(out.descent < 0.0);
    }

    #[test]
    fn empty_content_is_one_empty_line() {
        let out = laid("");
        assert!(out.glyphs.is_empty());
        assert_eq!(out.line_widths, vec![0.0]);
    }
}
