//! Rich text through the layout: the spans markup makes, and what the draw
//! takes from them.

use voltra_assets::{AssetPath, Fonts, DEFAULT_FONT};
use voltra_testkit::{copy_workspace_asset, scratch_root, NOTO_ARABIC};

use super::paragraph;
use super::shape::{glyphs, layout_at};
use super::*;

fn fonts() -> Fonts {
    let root = scratch_root();
    copy_workspace_asset(&root, DEFAULT_FONT);
    copy_workspace_asset(&root, NOTO_ARABIC);
    Fonts::new(root.keep())
}

fn rich(fonts: &mut Fonts, content: &str) -> Text {
    let mut text = Text {
        size: 1.0,
        pixels_per_em: 32,
        ..Text::new(content)
    };
    text.set_font(None, fonts);
    paragraph::load_fonts(&text, fonts);
    text
}

#[test]
fn plain_text_is_one_span() {
    let mut fonts = fonts();
    let text = rich(&mut fonts, "plain");
    let para = paragraph::of(&text, &fonts, fonts.default_font(), 32);
    assert_eq!(para.spans.len(), 1);
    assert_eq!(para.content, "plain");
}

#[test]
fn size_bold_and_font_tags_set_their_spans() {
    let mut fonts = fonts();
    let text = rich(
        &mut fonts,
        "a[size=2]b[/size][b]c[/b][font=fonts/NotoSansArabic-Regular.ttf]d[/font]",
    );
    let para = paragraph::of(&text, &fonts, fonts.default_font(), 32);
    assert_eq!(para.content, "abcd");
    let styles: Vec<_> = para.spans.iter().map(|s| s.style).collect();
    assert_eq!(
        styles[1].px, 64,
        "[size=2] on a size-1 text doubles the raster"
    );
    assert!(styles[2].embolden, "no bold file: faux bold");
    let arabic = fonts
        .by_path_handle(&AssetPath::new(NOTO_ARABIC).expect("a path"))
        .expect("loaded by load_fonts");
    assert_eq!(styles[3].font, arabic);
}

#[test]
fn a_bold_file_is_used_instead_of_faux_bold() {
    let mut fonts = fonts();
    let mut text = Text {
        bold_font: AssetPath::new(NOTO_ARABIC).ok(),
        ..rich(&mut fonts, "[b]x[/b]")
    };
    text.content = "[b]x[/b]".to_owned();
    paragraph::load_fonts(&text, &mut fonts);
    let para = paragraph::of(&text, &fonts, fonts.default_font(), 32);
    assert!(!para.spans[0].style.embolden);
    assert_ne!(para.spans[0].style.font, fonts.default_font());
}

#[test]
fn rich_off_shows_the_brackets() {
    let mut fonts = fonts();
    let mut text = rich(&mut fonts, "[b]x[/b]");
    text.rich = false;
    let para = paragraph::of(&text, &fonts, fonts.default_font(), 32);
    assert_eq!(para.content, "[b]x[/b]");
}

#[test]
fn a_run_s_colour_reaches_its_glyphs_and_the_text_s_alpha_fades_it() {
    let mut fonts = fonts();
    let mut text = rich(&mut fonts, "a[color=#00f]b[/color]");
    text.color = [1.0, 1.0, 1.0, 0.5];
    let layout = layout_at(&text, &fonts, text.px(), None).expect("laid out");
    fonts.rasterize(&layout);
    let (quads, _) = glyphs(&text, &fonts);
    assert_eq!(quads.len(), 2);
    assert_eq!(quads[0].color, [1.0, 1.0, 1.0, 0.5]);
    assert_eq!(quads[1].color, [0.0, 0.0, 1.0, 0.5]);
}

#[test]
fn a_wrapped_run_keeps_its_style_on_the_next_line() {
    let mut fonts = fonts();
    let mut text = rich(&mut fonts, "aa [color=red]bb cc dd[/color] ee");
    let one = layout_at(&text, &fonts, 32, None).expect("laid out");
    text.max_width = Some(one.width() / 32.0 * 0.4);
    let wrapped = layout_at(&text, &fonts, 32, text.max_width).expect("laid out");
    assert!(wrapped.line_widths.len() >= 3);
    let red_lines: std::collections::BTreeSet<usize> = wrapped
        .glyphs
        .iter()
        .filter(|g| g.tag == 1)
        .map(|g| g.line)
        .collect();
    assert!(
        red_lines.len() >= 2,
        "the red run spans lines: {red_lines:?}"
    );
}

#[test]
fn an_icon_takes_an_em_box_in_the_line() {
    let mut fonts = fonts();
    let text = rich(&mut fonts, "a[icon=ui/missing.png]b");
    let layout = layout_at(&text, &fonts, 32, None).expect("laid out");
    assert_eq!(layout.objects.len(), 1);
    let icon = layout.objects[0];
    assert!(icon.width > 10.0 && icon.height > 10.0);
    let b = layout.glyphs.iter().find(|g| g.cluster > 1).expect("b");
    assert!(b.pen >= icon.pen + icon.width - 1e-3, "b follows the icon");
}

#[test]
fn a_keyed_string_may_carry_markup() {
    let mut fonts = fonts();
    let mut text = rich(&mut fonts, "");
    text.content = "[b]Jugar[/b]".to_owned(); // as `Locale::localize` writes it
    let para = paragraph::of(&text, &fonts, fonts.default_font(), 32);
    assert_eq!(para.content, "Jugar");
    assert!(para.spans[0].style.embolden);
}
