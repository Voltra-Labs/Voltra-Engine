use super::*;

fn run_texts(markup: &Markup) -> Vec<&str> {
    markup
        .runs
        .iter()
        .map(|run| &markup.text[run.range.clone()])
        .collect()
}

#[test]
fn plain_text_is_one_unstyled_run() {
    let markup = parse("hello");
    assert_eq!(markup.text, "hello");
    assert_eq!(markup.runs.len(), 1);
    assert_eq!(markup.runs[0].style, Style::default());
}

#[test]
fn tags_come_out_and_style_their_runs() {
    let markup = parse("a [b]bold[/b] and [color=#f00]red[/color].");
    assert_eq!(markup.text, "a bold and red.");
    assert_eq!(run_texts(&markup), vec!["a ", "bold", " and ", "red", "."]);
    assert!(markup.runs[1].style.bold);
    assert_eq!(markup.runs[3].style.color, Some([1.0, 0.0, 0.0, 1.0]));
    assert_eq!(markup.runs[4].style, Style::default());
}

#[test]
fn tags_nest_and_close_in_any_order() {
    let markup = parse("[b][size=24]x[/b]y[/size]z");
    assert_eq!(run_texts(&markup), vec!["x", "y", "z"]);
    assert!(markup.runs[0].style.bold);
    assert_eq!(markup.runs[0].style.size, Some(24.0));
    assert!(!markup.runs[1].style.bold, "b closed first");
    assert_eq!(markup.runs[1].style.size, Some(24.0));
    assert_eq!(markup.runs[2].style, Style::default());
}

#[test]
fn colours_read_short_long_alpha_and_by_name() {
    assert_eq!(color("#fff"), Some([1.0; 4]));
    assert_eq!(
        color("#00ff0080").map(|c| (c[1], (c[3] * 255.0).round())),
        Some((1.0, 128.0))
    );
    assert_eq!(color("red"), Some([1.0, 0.0, 0.0, 1.0]));
    assert_eq!(color("#ggg"), None);
    assert_eq!(color("#12345"), None);
}

#[test]
fn unknown_tags_bad_values_and_stray_closes_are_shown() {
    for source in ["[wave]x", "[color=nope]x", "[size=-3]x", "x[/b]", "[b=2]x"] {
        let markup = parse(source);
        assert_eq!(markup.text, source, "{source} shown as written");
    }
}

#[test]
fn an_icon_is_one_object_character() {
    let markup = parse("coins: [icon=ui/items.atlas.ron/3] x3");
    assert_eq!(markup.text, format!("coins: {ICON_CHAR} x3"));
    let icon = markup
        .runs
        .iter()
        .find(|run| run.style.icon.is_some())
        .expect("an icon run");
    assert_eq!(&markup.text[icon.range.clone()], ICON_CHAR.to_string());
    assert_eq!(icon.style.icon.as_deref(), Some("ui/items.atlas.ron/3"));
}

#[test]
fn double_brackets_and_an_unclosed_bracket_are_literal() {
    assert_eq!(parse("[[b]] and [b").text, "[b]] and [b");
}

#[test]
fn a_font_tag_names_a_path() {
    let markup = parse("[font=fonts/NotoSansArabic-Regular.ttf]x[/font]");
    assert_eq!(
        markup.runs[0].style.font,
        AssetPath::new("fonts/NotoSansArabic-Regular.ttf").ok()
    );
}
