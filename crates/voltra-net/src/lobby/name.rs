//! A player's name, as a peer typed it, made safe to show and unique in
//! its lobby.

/// What a name that sanitises to nothing becomes.
const FALLBACK: &str = "Player";

/// `raw` made safe to show beside other players' names: control
/// characters (a newline, an escape) and the invisible formatting ones
/// (zero-width spaces and joiners, bidirectional overrides, a byte-order
/// mark) taken out, every run of whitespace one space, trimmed, and cut to
/// `max_chars` characters (at least one). A name with nothing left is
/// `"Player"`.
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
    let visible: String = raw
        .chars()
        .filter(|&c| !c.is_control() && !is_invisible_format(c))
        .collect();
    let mut text = String::new();
    for word in visible.split_whitespace() {
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(word);
    }
    let cut: String = text.chars().take(max_chars).collect();
    cut.trim_end().to_owned()
}

/// `name` if `taken` says no other player has it (case aside), else the
/// first of `name 2`, `name 3`, … that is free — the name cut so the
/// number still fits in `max_chars` (only the number, where even that
/// will not fit).
pub fn unique_name(name: &str, max_chars: usize, taken: impl Fn(&str) -> bool) -> String {
    let max_chars = max_chars.max(1);
    if !taken(name) {
        return name.to_owned();
    }
    for n in 2u32.. {
        let suffix = format!(" {n}");
        let room = max_chars.saturating_sub(suffix.chars().count());
        let base: String = name.chars().take(room).collect();
        let base = base.trim_end();
        let candidate = if base.is_empty() {
            n.to_string()
        } else {
            format!("{base}{suffix}")
        };
        if !taken(&candidate) {
            return candidate;
        }
    }
    // A lobby holds far fewer players than there are numbers.
    name.to_owned()
}

/// Whether two names are the same to a player reading them.
pub(crate) fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// Characters that draw nothing but change how the text around them
/// reads: zero-width spaces and joiners, bidirectional embeddings,
/// overrides and isolates, invisible operators, the byte-order mark.
fn is_invisible_format(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'
            | '\u{034F}'
            | '\u{061C}'
            | '\u{115F}'
            | '\u{1160}'
            | '\u{17B4}'
            | '\u{17B5}'
            | '\u{180E}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{206F}'
            | '\u{3164}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{FEFF}'
            | '\u{FFA0}'
            | '\u{FFF0}'..='\u{FFF8}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_and_invisible_characters_are_taken_out() {
        assert_eq!(sanitize_name("Ada\nLovelace", 32), "AdaLovelace");
        assert_eq!(sanitize_name("\u{1b}[31mred", 32), "[31mred");
        assert_eq!(sanitize_name("a\u{202E}gnp.exe", 32), "agnp.exe");
        assert_eq!(sanitize_name("zero\u{200B}width", 32), "zerowidth");
        assert_eq!(sanitize_name("  two   words\t ", 32), "two words");
        assert_eq!(sanitize_name("Zoë 🦀", 32), "Zoë 🦀");
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
    fn a_taken_name_gets_the_first_free_number_and_still_fits() {
        let names = ["Ada", "ada 2", "Bob"];
        let taken = |name: &str| names.iter().any(|n| same_name(n, name));

        assert_eq!(unique_name("Cy", 16, taken), "Cy");
        assert_eq!(unique_name("ADA", 16, taken), "ADA 3");
        assert_eq!(unique_name("Bob", 4, taken), "Bo 2");
        assert_eq!(unique_name("B", 1, |n: &str| n == "B"), "2");
    }
}
