//! Shaping and the fallback chain, in the faces the workspace ships.

use super::*;
use voltra_testkit::{
    copy_workspace_asset, scratch_root, NOTO_ARABIC, NOTO_DEVANAGARI, NOTO_HEBREW,
};

fn path(raw: &str) -> AssetPath {
    AssetPath::new(raw).expect("a valid asset path")
}

fn fonts() -> Fonts {
    let root = scratch_root();
    for font in [DEFAULT_FONT, NOTO_ARABIC, NOTO_DEVANAGARI, NOTO_HEBREW] {
        copy_workspace_asset(&root, font);
    }
    Fonts::new(root)
}

/// Fonts falling back to Arabic, Devanagari and Hebrew, as a locale table
/// naming them sets up.
fn chained() -> Fonts {
    let mut fonts = fonts();
    fonts.set_fallbacks(&[path(NOTO_ARABIC), path(NOTO_DEVANAGARI), path(NOTO_HEBREW)]);
    fonts
}

fn laid(fonts: &Fonts, content: &str) -> Arc<TextLayout> {
    fonts
        .layout_plain(fonts.default_font(), content, 32, Wrap::default())
        .expect("a face")
}

fn ids(layout: &TextLayout) -> Vec<u16> {
    layout.glyphs.iter().map(|g| g.key.glyph).collect()
}

fn handle(fonts: &Fonts, raw: &str) -> Handle<Font> {
    fonts.by_path_handle(&path(raw)).expect("loaded")
}

#[test]
fn ffi_is_one_ligature_with_a_caret_stop_per_letter() {
    let fonts = fonts();
    let ffi = laid(&fonts, "ffi");
    assert_eq!(ffi.glyphs.len(), 1, "f, f and i become one glyph");
    assert_ne!(ids(&ffi), ids(&laid(&fonts, "f"))[..1]);

    let stops: Vec<f32> = ffi.carets.iter().map(|c| c.x).collect();
    assert_eq!(stops.len(), 4);
    assert!(stops.windows(2).all(|w| w[1] > w[0]), "{stops:?}");
    assert!((stops[3] - ffi.width()).abs() < 1e-3);
}

#[test]
fn arabic_letters_join_and_read_right_to_left() {
    let fonts = chained();
    let arabic = handle(&fonts, NOTO_ARABIC);
    let word = laid(&fonts, "سلام");
    assert!(word.glyphs.iter().all(|g| g.key.font == arabic.index()));

    // Each letter alone is its isolated form; in the word, joined forms.
    let alone: Vec<u16> = ["س", "ل", "ا", "م"]
        .iter()
        .map(|letter| ids(&laid(&fonts, letter))[0])
        .collect();
    let joined = ids(&word);
    let changed = alone.iter().filter(|id| !joined.contains(id)).count();
    assert!(changed >= 2, "joined {joined:?}, isolated {alone:?}");

    // Logical order runs right to left: the first letter is drawn last.
    let first = word
        .glyphs
        .iter()
        .find(|g| g.cluster == 0)
        .expect("the seen");
    assert!(word.glyphs.iter().all(|g| g.pen <= first.pen));
    let start = word.carets.first().expect("a stop before the first letter");
    assert!(
        (start.x - word.width()).abs() < 1e-3,
        "its stop is on the right"
    );
    assert!(word.carets.last().expect("an end stop").x.abs() < 1e-3);
}

#[test]
fn a_devanagari_i_sign_is_drawn_before_its_consonant() {
    let fonts = chained();
    let ka = ids(&laid(&fonts, "क"))[0];
    let ki = laid(&fonts, "कि");
    assert_eq!(ki.glyphs.len(), 2);
    assert_ne!(ki.glyphs[0].key.glyph, ka, "the i sign comes first");
    assert_eq!(ki.glyphs[1].key.glyph, ka);
    assert!(ki.glyphs.iter().all(|g| g.cluster == 0), "one cluster");
}

#[test]
fn a_glyph_the_text_font_lacks_comes_from_the_next_font() {
    let fonts = chained();
    let (latin, arabic) = (fonts.default_font(), handle(&fonts, NOTO_ARABIC));
    let mixed = laid(&fonts, "a س b");
    let by_cluster = |byte: usize| {
        mixed
            .glyphs
            .iter()
            .find(|g| g.cluster == byte)
            .map(|g| g.key.font)
    };
    assert_eq!(by_cluster(0), Some(latin.index()));
    assert_eq!(by_cluster(2), Some(arabic.index()));
    assert_eq!(by_cluster(5), Some(latin.index()));
    assert!(mixed.glyphs.iter().all(|g| g.key.glyph != 0));
}

#[test]
fn a_glyph_no_font_has_is_the_text_fonts_missing_box() {
    let fonts = chained();
    let cjk = laid(&fonts, "日本");
    assert_eq!(cjk.glyphs.len(), 2);
    assert!(cjk
        .glyphs
        .iter()
        .all(|g| g.key.glyph == 0 && g.key.font == fonts.default_font().index()));
}

#[test]
fn with_no_fallbacks_arabic_is_missing_boxes() {
    let fonts = fonts();
    assert!(ids(&laid(&fonts, "سلام")).iter().all(|id| *id == 0));
}

#[test]
fn latin_is_still_kerned() {
    let fonts = fonts();
    let alone = laid(&fonts, "A").width();
    let pair = laid(&fonts, "AV");
    assert!(pair.glyphs[1].pen <= alone + 1e-3);
}

#[test]
fn a_layout_is_made_once_and_a_new_chain_makes_it_again() {
    let mut fonts = fonts();
    let first = laid(&fonts, "hello");
    assert!(Arc::ptr_eq(&first, &laid(&fonts, "hello")));
    assert_eq!(fonts.layouts.len(), 1);

    fonts.set_fallbacks(&[path(NOTO_ARABIC)]);
    assert_eq!(fonts.layouts.len(), 0);
    let again = laid(&fonts, "hello");
    assert!(!Arc::ptr_eq(&first, &again));
    assert_eq!(*first, *again, "same fonts, same layout");
}

#[test]
fn faux_bold_widens_the_advance_and_thickens_the_glyph() {
    let mut fonts = fonts();
    let font = fonts.default_font();
    let style = |embolden| RunStyle {
        embolden,
        ..RunStyle::plain(font, 32)
    };
    let regular = fonts
        .layout("ll", &[Span::whole("ll", style(false))], Wrap::default())
        .expect("a face");
    let bold = fonts
        .layout("ll", &[Span::whole("ll", style(true))], Wrap::default())
        .expect("a face");
    assert!(bold.width() > regular.width());
    assert!(bold.glyphs.iter().all(|g| g.key.embolden));

    fonts.rasterize(&regular);
    fonts.rasterize(&bold);
    let inked = |key| fonts.atlas().get(key).expect("rasterized").metrics.width;
    assert!(inked(bold.glyphs[0].key) > inked(regular.glyphs[0].key));
}

#[test]
fn rasterize_takes_each_glyph_from_its_own_font() {
    let mut fonts = chained();
    let mixed = laid(&fonts, "aسक");
    fonts.rasterize(&mixed);
    for glyph in &mixed.glyphs {
        let stored = fonts.atlas().get(glyph.key).expect("rasterized");
        assert!(stored.rect.is_some(), "{:?} has ink", glyph.key);
    }
    assert_eq!(fonts.atlas().len(), 3);
}
