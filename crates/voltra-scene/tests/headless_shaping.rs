//! Shaped text in pixels: Arabic joined and right to left, Devanagari with
//! its vowel sign moved before the consonant, an `ffi` ligature — each
//! matched against a reference image — and a Latin line that draws as it
//! did before shaping.
//!
//! Skips itself when no GPU adapter is available.

#[path = "support/text_shot.rs"]
mod text_shot;

use text_shot::{inked, matches_reference, Shot};
use voltra_assets::DEFAULT_FONT;
use voltra_render::glam::Vec2;
use voltra_scene::Text;
use voltra_testkit::{
    copy_workspace_asset, scratch_root, NOTO_ARABIC, NOTO_DEVANAGARI, NOTO_HEBREW,
};

const W: u32 = 192;
const H: u32 = 48;

fn shot() -> Shot {
    let root = scratch_root().keep();
    for font in [DEFAULT_FONT, NOTO_ARABIC, NOTO_DEVANAGARI, NOTO_HEBREW] {
        copy_workspace_asset(&root, font);
    }
    Shot::new(root, &[NOTO_ARABIC, NOTO_DEVANAGARI, NOTO_HEBREW], W, H)
}

fn line(content: &str) -> Text {
    Text {
        size: 24.0,
        color: [1.0, 1.0, 1.0, 1.0],
        ..Text::new(content)
    }
}

fn drawn(content: &str) -> Option<Vec<voltra_testkit::Rgba>> {
    let mut shot = shot();
    shot.text(Vec2::new(8.0, 8.0), Vec2::new(176.0, 32.0), line(content));
    let pixels = shot.draw();
    if pixels.is_none() {
        eprintln!("no GPU adapter; skipping");
    }
    pixels
}

#[test]
fn arabic_draws_joined_right_to_left() {
    let Some(pixels) = drawn("مرحبا بالعالم") else {
        return;
    };
    assert!(inked(&pixels) > 200, "the line draws");
    matches_reference("shaping_arabic", &pixels, W, H);
}

#[test]
fn devanagari_draws_its_conjuncts_and_vowel_signs() {
    let Some(pixels) = drawn("नमस्ते किताब") else {
        return;
    };
    assert!(inked(&pixels) > 200, "the line draws");
    matches_reference("shaping_devanagari", &pixels, W, H);
}

#[test]
fn ffi_draws_as_a_ligature() {
    let Some(pixels) = drawn("office ffi") else {
        return;
    };
    assert!(inked(&pixels) > 200, "the line draws");
    matches_reference("shaping_ffi", &pixels, W, H);
}

#[test]
fn a_shaped_line_differs_from_its_letters_drawn_apart() {
    // The same Arabic letters with zero-width non-joiners between them
    // stay in their isolated forms: if shaping did nothing, the two frames
    // would match.
    let (Some(joined), Some(apart)) = (drawn("سلام"), drawn("س\u{200C}ل\u{200C}ا\u{200C}م"))
    else {
        return;
    };
    let differing = joined
        .iter()
        .zip(&apart)
        .filter(|(a, b)| a.is_clear_ish() != b.is_clear_ish())
        .count();
    assert!(differing > 50, "{differing} pixels differ");
}
