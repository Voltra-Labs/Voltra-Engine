//! Names as hostile peers type them.

use super::*;

#[test]
fn control_and_invisible_characters_are_taken_out() {
    assert_eq!(
        sanitize_name("Ada\nLovelace", 32),
        "Ada Lovelace",
        "a newline is a space"
    );
    assert_eq!(sanitize_name("\u{1b}[31mred", 32), "[31mred");
    assert_eq!(sanitize_name("a\u{202E}gnp.exe", 32), "agnp.exe");
    assert_eq!(sanitize_name("zero\u{200B}width", 32), "zerowidth");
    assert_eq!(sanitize_name("  two   words\t ", 32), "two words");
    assert_eq!(sanitize_name("Zoë 🦀", 32), "Zoë 🦀");
}

#[test]
fn every_format_character_goes_not_only_a_listed_few() {
    // Cf characters no hand-written list had: an isolate, interlinear
    // annotation, a Kaithi number sign, a language tag, the Mongolian
    // vowel separator; and a private-use and an unassigned code point.
    for hidden in [
        '\u{2066}',
        '\u{FFF9}',
        '\u{110BD}',
        '\u{E0001}',
        '\u{E0041}',
        '\u{180E}',
        '\u{E000}',
        '\u{0378}',
    ] {
        assert_eq!(
            sanitize_name(&format!("A{hidden}da"), 32),
            "Ada",
            "{hidden:?}"
        );
    }
}

#[test]
fn an_emoji_keeps_its_joiner_and_selector_and_a_letter_does_not() {
    let coder = "👩\u{200D}💻";
    assert_eq!(sanitize_name(coder, 32), coder);
    let heart = "❤\u{FE0F}";
    assert_eq!(sanitize_name(heart, 32), heart);
    let keycap = "1\u{FE0F}\u{20E3}";
    assert_eq!(sanitize_name(keycap, 32), keycap);
    let waving = "👋\u{1F3FD}\u{200D}🔥";
    assert_eq!(sanitize_name(waving, 32), waving);

    assert_eq!(sanitize_name("a\u{200D}b", 32), "ab");
    assert_eq!(sanitize_name("a\u{FE0F}", 32), "a");
    assert_eq!(sanitize_name("\u{200D}👩", 32), "👩", "nothing to join");
    assert_eq!(sanitize_name("👩\u{200D}", 32), "👩", "nothing joined");
    assert_eq!(sanitize_name("👩\u{200D}💻", 2), "👩", "cut mid-sequence");
}

#[test]
fn a_name_is_composed_before_it_is_cut() {
    // Five decomposed é: cut to three characters, three whole é.
    let decomposed = "e\u{301}".repeat(5);
    assert_eq!(sanitize_name(&decomposed, 3), "\u{e9}".repeat(3));
}

#[test]
fn combining_marks_are_capped_per_base() {
    let zalgo = format!("Z{}a{}", "\u{0336}".repeat(40), "\u{0489}".repeat(40));
    let name = sanitize_name(&zalgo, 64);
    let marks: Vec<usize> = name
        .split(|c: char| !is_mark(c))
        .skip(1)
        .map(|run| run.chars().count())
        .collect();
    assert_eq!(marks, [MAX_MARKS_PER_BASE, MAX_MARKS_PER_BASE], "{name:?}");
    assert!(name.starts_with('Z') && name.contains('a'));

    // Real stacking survives: a Vietnamese vowel with two marks.
    assert_eq!(sanitize_name("Nguyê\u{0303}n", 16), "Nguyễn");
    // A mark with no base is dropped.
    assert_eq!(sanitize_name("\u{0301}\u{0301}Bo", 16), "Bo");
    assert_eq!(
        sanitize_name("a \u{0301}b", 16),
        "a b",
        "a space is no base"
    );
}

#[test]
fn a_long_name_is_cut_by_characters_not_bytes() {
    assert_eq!(sanitize_name("abcdefghij", 4), "abcd");
    assert_eq!(sanitize_name("ééééé", 3), "ééé");
    assert_eq!(sanitize_name("ab   cd", 3), "ab", "no trailing space");
}

#[test]
fn nothing_left_is_a_fallback() {
    assert_eq!(sanitize_name("", 16), "Player");
    assert_eq!(sanitize_name("\n\t\u{200B} ", 16), "Player");
    assert_eq!(sanitize_name("", 3), "Pla");
    assert_eq!(sanitize_name("x", 0), "x", "at least one character");
}

#[test]
fn names_read_alike_are_the_same_name() {
    assert!(same_name("Ada", "ADA"));
    assert!(same_name("ada", "ＡＤＡ"), "fullwidth");
    assert!(same_name("STRASSE", "straße"), "full case folding");
    assert!(same_name("ﬁsh", "FISH"), "a ligature");
    assert!(same_name("é", "e\u{301}"), "composed or not");
    assert!(same_name("❤\u{FE0F}", "❤"), "the emoji selector aside");
    assert!(same_name("Ǆ", "dž"), "titlecase digraphs");
    assert!(!same_name("Ada", "Adam"));
    assert!(!same_name("Ada", "Ada 2"));
}

#[test]
fn a_taken_name_gets_the_first_free_number_and_still_fits() {
    let names = ["Ada", "ada 2", "Bob"];
    let taken = |name: &str| names.iter().any(|n| same_name(n, name));

    assert_eq!(unique_name("Cy", 16, taken), "Cy");
    assert_eq!(unique_name("ADA", 16, taken), "ADA 3");
    assert_eq!(unique_name("ＡＤＡ", 16, taken), "ＡＤＡ 3");
    assert_eq!(unique_name("Bob", 4, taken), "Bo 2");
    assert_eq!(unique_name("B", 1, |n: &str| n == "B"), "2");
}

#[test]
fn a_one_character_name_never_grows_past_its_cap() {
    // Every one-character number taken: "10" does not fit, so the name
    // stays as long as it may, if no longer unique.
    let taken = |name: &str| name == "B" || (name.len() == 1 && name != "x");
    let name = unique_name("B", 1, taken);
    assert!(name.chars().count() <= 1, "{name:?}");
    assert_eq!(unique_name("Bob", 1, |n: &str| n == "B"), "2", "cut first");
}

#[test]
fn blank_lookalikes_are_taken_out() {
    for blank in [
        '\u{2800}',
        '\u{3164}',
        '\u{115F}',
        '\u{1160}',
        '\u{FFA0}',
        '\u{FFFC}',
        '\u{16FE4}',
        '\u{1D159}',
    ] {
        assert_eq!(
            sanitize_name(&format!("Ada{blank}"), 16),
            "Ada",
            "{blank:?}"
        );
        assert_eq!(
            sanitize_name(&format!("{blank}{blank}"), 16),
            "Player",
            "{blank:?}: a blank name is no name"
        );
    }
    // A Braille pattern with dots is a letter like any other.
    assert_eq!(sanitize_name("\u{2801}", 16), "\u{2801}");
}
