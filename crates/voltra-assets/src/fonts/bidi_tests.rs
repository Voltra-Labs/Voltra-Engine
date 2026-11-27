//! Bidirectional order and Unicode line breaking, pinned on layouts.

use super::*;
use voltra_testkit::{copy_workspace_asset, scratch_root, NOTO_ARABIC, NOTO_HEBREW};

fn fonts() -> Fonts {
    let root = scratch_root().keep();
    for font in [DEFAULT_FONT, NOTO_ARABIC, NOTO_HEBREW] {
        copy_workspace_asset(&root, font);
    }
    let mut fonts = Fonts::new(root);
    let fallbacks = [NOTO_HEBREW, NOTO_ARABIC].map(|p| AssetPath::new(p).expect("a path"));
    fonts.set_fallbacks(&fallbacks);
    fonts
}

fn laid(fonts: &Fonts, content: &str, wrap: Wrap) -> Arc<TextLayout> {
    fonts
        .layout_plain(fonts.default_font(), content, 32, wrap)
        .expect("a face")
}

/// The first byte of each glyph's cluster, left to right.
fn visual(layout: &TextLayout, line: usize) -> Vec<usize> {
    layout
        .glyphs
        .iter()
        .filter(|g| g.line == line)
        .map(|g| g.cluster)
        .collect()
}

#[test]
fn hebrew_inside_english_reads_right_to_left_and_the_english_stays_put() {
    let fonts = fonts();
    // "a" (0), " " (1), shin (2), lamed (4), vav (6), final mem (8),
    // " " (10), "b" (11).
    let mixed = laid(&fonts, "a שלום b", Wrap::default());
    assert_eq!(visual(&mixed, 0), vec![0, 1, 8, 6, 4, 2, 10, 11]);
    let pens: Vec<f32> = mixed.glyphs.iter().map(|g| g.pen).collect();
    assert!(pens.windows(2).all(|w| w[1] > w[0] - 1.0), "{pens:?}");
}

#[test]
fn english_inside_hebrew_keeps_its_own_order() {
    let fonts = fonts();
    // A right-to-left paragraph (it starts with Hebrew): the Hebrew word
    // is on the right, "ok" on its left reads left to right.
    let mixed = laid(&fonts, "שלום ok", Wrap::default());
    let order = visual(&mixed, 0);
    // "o" (9) before "k" (10), both left of the Hebrew.
    assert_eq!(&order[..2], &[9, 10], "{order:?}");
    assert_eq!(
        order.last(),
        Some(&0),
        "the first Hebrew letter is rightmost"
    );
}

#[test]
fn numbers_in_arabic_keep_their_digit_order() {
    let fonts = fonts();
    let mixed = laid(&fonts, "عام 2024", Wrap::default());
    let order = visual(&mixed, 0);
    // Three letters of two bytes, a space at 6, the digits from 7.
    let digits: Vec<usize> = order.iter().copied().filter(|b| *b >= 7).collect();
    assert_eq!(digits, vec![7, 8, 9, 10], "2, 0, 2, 4 left to right");
    assert!(order.iter().position(|b| *b == 7) < order.iter().position(|b| *b == 0));
}

#[test]
fn caret_stops_follow_the_letters_in_mixed_text() {
    let fonts = fonts();
    let mixed = laid(&fonts, "ab שלום", Wrap::default());
    let x = |byte: usize| {
        mixed
            .carets
            .iter()
            .find(|c| c.byte == byte)
            .map(|c| c.x)
            .expect("a stop")
    };
    // Left to right through "ab", then the Hebrew runs leftward from its
    // right end.
    assert!(x(0) < x(1) && x(1) < x(2));
    assert!(x(3) > x(5) && x(5) > x(7) && x(7) > x(9));
    assert!(
        (x(3) - mixed.width()).abs() < 1e-3,
        "shin stands at the right end"
    );
    assert_eq!(mixed.carets.len(), "ab שלום".chars().count() + 1);
}

#[test]
fn japanese_wraps_between_characters_inside_its_width() {
    let fonts = fonts();
    let text = "日本語の文章は空白なしで折り返す";
    let one = laid(&fonts, text, Wrap::default());
    let max = one.width() / 3.0;
    let wrapped = laid(
        &fonts,
        text,
        Wrap {
            max_width: Some(max),
            ..Wrap::default()
        },
    );
    assert!(wrapped.line_widths.len() >= 3, "{:?}", wrapped.line_widths);
    assert!(wrapped.line_widths.iter().all(|w| *w <= max + 1e-3));
    assert_eq!(
        wrapped.glyphs.len(),
        text.chars().count(),
        "nothing dropped"
    );
}

#[test]
fn japanese_punctuation_does_not_start_a_line() {
    let fonts = fonts();
    // UAX #14 keeps 。 with the character before it.
    let text = "日本。日本";
    let advance = laid(&fonts, "日", Wrap::default()).width();
    let wrapped = laid(
        &fonts,
        text,
        Wrap {
            max_width: Some(advance * 2.5),
            ..Wrap::default()
        },
    );
    let starts: Vec<usize> = (0..wrapped.line_widths.len())
        .filter_map(|line| visual(&wrapped, line).first().copied())
        .collect();
    assert!(!starts.contains(&6), "a line starts with 。: {starts:?}");
}

#[test]
fn a_hyphen_is_a_break_opportunity() {
    let fonts = fonts();
    let one = laid(&fonts, "well-known", Wrap::default());
    // Just past the hyphen: "well-" fits, "well-k" does not.
    let hyphen = one.glyphs[5].pen + 1.0;
    let wrapped = laid(
        &fonts,
        "well-known",
        Wrap {
            max_width: Some(hyphen),
            ..Wrap::default()
        },
    );
    // "known" may itself be too wide and break between letters; the first
    // break is after the hyphen all the same.
    assert_eq!(
        visual(&wrapped, 0),
        vec![0, 1, 2, 3, 4],
        "\"well-\" is line 1"
    );
    assert_eq!(
        visual(&wrapped, 1).first(),
        Some(&5),
        "\"known\" starts line 2"
    );
}

#[test]
fn a_wrapped_rtl_paragraph_keeps_its_first_words_on_the_first_line() {
    let fonts = fonts();
    let text = "שלום עולם שלום";
    let one = laid(&fonts, text, Wrap::default());
    let wrapped = laid(
        &fonts,
        text,
        Wrap {
            max_width: Some(one.width() * 0.7),
            ..Wrap::default()
        },
    );
    assert_eq!(wrapped.line_widths.len(), 2);
    assert!(
        visual(&wrapped, 0).contains(&0),
        "the first word is on line 1"
    );
    assert!(!visual(&wrapped, 1).contains(&0));
}
