//! A player's name, as a peer typed it, made safe to show and unique in
//! its lobby.
//!
//! Untrusted text shown beside other players' names. It must not draw
//! what it is not (a bidirectional override turning `exe.png` around, a
//! zero-width character making two names look alike), nor draw more than
//! its line (a base under dozens of stacked combining marks), and two
//! names a player reads as one ("ADA", "ada", "ＡＤＡ") are one name.
//! Unicode's own tools do it: the general category to find every format
//! character, NFC so a name is cut between characters as typed, and the
//! compatibility caseless match (NFKC with full case folding, D145) to
//! compare two.

use caseless::Caseless;
use unicode_general_category::{get_general_category, GeneralCategory};
use unicode_normalization::UnicodeNormalization;

/// What a name that sanitises to nothing becomes.
const FALLBACK: &str = "Player";

/// Combining marks kept on one base character: enough for every script's
/// own stacking (a Vietnamese vowel takes two), too few to draw a name
/// over the lines around it.
pub const MAX_MARKS_PER_BASE: usize = 3;

/// The zero-width joiner: kept only between two emoji, where it makes one
/// ("👩‍💻").
const ZWJ: char = '\u{200D}';

/// Variation selector 16: kept only after an emoji-capable symbol, which
/// it asks to draw as one ("❤️"), or a keycap's digit ("1️⃣").
const VS16: char = '\u{FE0F}';

/// `raw` made safe to show beside other players' names: control
/// characters, format characters (zero-width ones, bidirectional
/// overrides and isolates, a byte-order mark, tags — every one of general
/// category Cf, but a joiner inside an emoji sequence and the selector
/// that makes a symbol an emoji), private-use and unassigned characters
/// and the few other ones that draw nothing taken out (a newline or a tab
/// is whitespace, so a space); composed (NFC);
/// combining marks past [`MAX_MARKS_PER_BASE`] on one base dropped; every
/// run of whitespace one space, trimmed; and cut to `max_chars`
/// characters (at least one). A name with nothing left is `"Player"`.
pub fn sanitize_name(raw: &str, max_chars: usize) -> String {
    let max_chars = max_chars.max(1);
    let name = visible_text(raw, max_chars);
    if name.is_empty() {
        FALLBACK.chars().take(max_chars).collect()
    } else {
        name
    }
}

/// [`sanitize_name`] with nothing put in for an empty name: what a server
/// called itself, a game id.
pub(crate) fn visible_text(raw: &str, max_chars: usize) -> String {
    let drawn: String = drawn_chars(raw).nfc().collect();
    let capped = cap_marks(&drawn);
    let mut text = String::new();
    for word in capped.split_whitespace() {
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(word);
    }
    let mut cut: String = text.chars().take(max_chars).collect();
    // A cut through an emoji sequence leaves a joiner with nothing after.
    while cut.ends_with(ZWJ) || cut.ends_with(char::is_whitespace) {
        cut.pop();
    }
    cut
}

/// `raw`'s characters that draw, or join two that do.
fn drawn_chars(raw: &str) -> impl Iterator<Item = char> + '_ {
    let mut chars = raw.chars().peekable();
    let mut previous: Option<char> = None;
    std::iter::from_fn(move || loop {
        let c = chars.next()?;
        let keep = match c {
            ZWJ => {
                previous.is_some_and(is_emoji_part) && chars.peek().copied().is_some_and(is_emoji)
            }
            VS16 => previous.is_some_and(|p| is_emoji(p) || matches!(p, '0'..='9' | '#' | '*')),
            _ => draws(c),
        };
        if keep {
            previous = Some(c);
            return Some(c);
        }
    })
}

/// Whether `c` is a character that draws, or is whitespace: not a control,
/// a format character, a private-use, unassigned or surrogate code point,
/// nor one of the few others that draw nothing.
fn draws(c: char) -> bool {
    if c.is_whitespace() {
        return true;
    }
    !matches!(
        get_general_category(c),
        GeneralCategory::Control
            | GeneralCategory::Format
            | GeneralCategory::PrivateUse
            | GeneralCategory::Unassigned
            | GeneralCategory::Surrogate
    ) && !is_blank_letter(c)
}

/// Characters of another category that draw nothing, or only a blank a
/// name could hide behind ("Ada⠀" for "Ada", "⠀⠀" for no name): the
/// combining grapheme joiner, the Khmer inherent vowels, the Mongolian
/// free variation selectors, the Hangul fillers (U+115F, U+1160, U+3164,
/// U+FFA0), the blank Braille pattern (U+2800), the object replacement
/// character (rich text's icon slot), the Khitan filler, the musical null
/// notehead, and the variation selectors other than [`VS16`].
fn is_blank_letter(c: char) -> bool {
    matches!(
        c,
        '\u{034F}'
            | '\u{115F}'
            | '\u{1160}'
            | '\u{17B4}'
            | '\u{17B5}'
            | '\u{180B}'..='\u{180D}'
            | '\u{180F}'
            | '\u{2800}'
            | '\u{3164}'
            | '\u{FE00}'..='\u{FE0E}'
            | '\u{FFA0}'
            | '\u{FFFC}'
            | '\u{16FE4}'
            | '\u{1D159}'
            | '\u{E0100}'..='\u{E01EF}'
    )
}

/// A symbol an emoji sequence is built of (most emoji are Other Symbol).
fn is_emoji(c: char) -> bool {
    get_general_category(c) == GeneralCategory::OtherSymbol
}

/// What may stand before a joiner in an emoji sequence: an emoji, its
/// skin-tone modifier, its emoji selector.
fn is_emoji_part(c: char) -> bool {
    is_emoji(c) || c == VS16 || matches!(c, '\u{1F3FB}'..='\u{1F3FF}')
}

fn is_mark(c: char) -> bool {
    matches!(
        get_general_category(c),
        GeneralCategory::NonspacingMark
            | GeneralCategory::SpacingMark
            | GeneralCategory::EnclosingMark
    )
}

/// `text` with at most [`MAX_MARKS_PER_BASE`] combining marks after each
/// base, and none before the first.
fn cap_marks(text: &str) -> String {
    let mut capped = String::with_capacity(text.len());
    let mut marks: Option<usize> = None;
    for c in text.chars() {
        if is_mark(c) {
            match marks.as_mut() {
                Some(count) if *count < MAX_MARKS_PER_BASE => {
                    *count += 1;
                    capped.push(c);
                }
                _ => {}
            }
        } else {
            // A space carries no marks; anything else is a base.
            marks = (!c.is_whitespace()).then_some(0);
            capped.push(c);
        }
    }
    capped
}

/// `name` if `taken` says no other player has it, else the first of
/// `name 2`, `name 3`, … that is free — the name cut so the number still
/// fits in `max_chars`, or only the number where even that will not. The
/// answer is never longer than `max_chars`; with every number that fits
/// taken it is `name` cut, not unique (a lobby's
/// [`max_name_chars`](crate::LobbyConfig::max_name_chars) always leaves
/// room for its slots' numbers).
pub fn unique_name(name: &str, max_chars: usize, taken: impl Fn(&str) -> bool) -> String {
    let max_chars = max_chars.max(1);
    let name: String = name.chars().take(max_chars).collect();
    if !taken(&name) {
        return name;
    }
    for n in 2u32.. {
        let number = n.to_string();
        if number.len() > max_chars {
            break;
        }
        let suffix = format!(" {number}");
        let room = max_chars.saturating_sub(suffix.chars().count());
        let base: String = name.chars().take(room).collect();
        let base = base.trim_end();
        let candidate = if base.is_empty() {
            number
        } else {
            format!("{base}{suffix}")
        };
        if !taken(&candidate) {
            return candidate;
        }
    }
    name
}

/// Whether two names are the same to a player reading them: equal under
/// the compatibility caseless match (NFKC, full case folding — "ﬁ" is
/// "fi", "ß" is "ss", "Ａ" is "a"), the emoji joiner and selector aside.
pub(crate) fn same_name(a: &str, b: &str) -> bool {
    let key = |name: &str| {
        name.chars()
            .filter(|&c| c != ZWJ && c != VS16)
            .nfd()
            .default_case_fold()
            .nfkd()
            .default_case_fold()
            .nfkd()
            .collect::<Vec<char>>()
    };
    key(a) == key(b)
}

#[cfg(test)]
mod tests;
