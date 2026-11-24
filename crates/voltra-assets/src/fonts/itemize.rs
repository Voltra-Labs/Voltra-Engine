//! A paragraph cut into runs the shaper takes one at a time: each one span,
//! one bidi level and one script.

use std::ops::Range;

use unicode_bidi::Level;
use unicode_script::{Script, UnicodeScript};

use super::span::{span_at, Span};

/// Bytes that shape together.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Run {
    /// Bytes of the whole laid-out string.
    pub range: Range<usize>,
    /// Index of the span it is in.
    pub span: usize,
    /// Its bidi embedding level: odd is right to left.
    pub level: Level,
    /// Its script, or `None` when it holds only neutral characters
    /// (spaces, digits, punctuation) with no script around them.
    pub script: Option<rustybuzz::Script>,
}

/// Cuts `paragraph` — bytes `base..` of the string `spans` index — into
/// runs, given each of its bytes' bidi `levels`.
///
/// Characters of the Common and Inherited scripts (spaces, punctuation, a
/// combining mark) join the script before them, or the one after at the
/// start: a space between two Arabic words is shaped as Arabic, and a mark
/// stays with its base. HarfBuzz's own `hb-ot-shape` itemization rule,
/// which `rustybuzz` leaves to its caller.
pub(crate) fn runs(paragraph: &str, base: usize, spans: &[Span], levels: &[Level]) -> Vec<Run> {
    let scripts = resolved_scripts(paragraph);
    let mut runs: Vec<Run> = Vec::new();
    for ((offset, _), script) in paragraph.char_indices().zip(scripts) {
        let at = base + offset;
        let span = span_at(spans, at);
        let level = levels.get(offset).copied().unwrap_or_else(Level::ltr);
        let object = spans[span].style.object.is_some();
        if let Some(run) = runs.last_mut() {
            let same = run.span == span && run.level == level && run.script == script && !object;
            if same {
                run.range.end = at + char_len(paragraph, offset);
                continue;
            }
        }
        runs.push(Run {
            range: at..at + char_len(paragraph, offset),
            span,
            level,
            script,
        });
    }
    runs
}

fn char_len(text: &str, at: usize) -> usize {
    text[at..].chars().next().map_or(1, char::len_utf8)
}

/// Each character's script, the neutral ones given their neighbours'.
fn resolved_scripts(text: &str) -> Vec<Option<rustybuzz::Script>> {
    let raw: Vec<Option<rustybuzz::Script>> = text.chars().map(script_of).collect();
    let mut out = Vec::with_capacity(raw.len());
    let mut current = raw.iter().flatten().next().copied();
    for script in raw {
        if let Some(script) = script {
            current = Some(script);
        }
        out.push(current);
    }
    out
}

/// A character's script as the shaper names it, or `None` for one that
/// takes its neighbours' (Common, Inherited, Unknown).
fn script_of(character: char) -> Option<rustybuzz::Script> {
    let script = character.script();
    if matches!(script, Script::Common | Script::Inherited | Script::Unknown) {
        return None;
    }
    let name: [u8; 4] = script.short_name().as_bytes().try_into().ok()?;
    rustybuzz::Script::from_iso15924_tag(rustybuzz::ttf_parser::Tag::from_bytes(&name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::span::RunStyle;
    use crate::handle::Handle;

    fn one_span(text: &str) -> Vec<Span> {
        vec![Span::whole(text, RunStyle::plain(Handle::forge(0, 0), 16))]
    }

    #[test]
    fn a_change_of_script_starts_a_run_and_spaces_join_the_one_before() {
        let text = "ab नमस्ते cd";
        let levels = vec![Level::ltr(); text.len()];
        let runs = runs(text, 0, &one_span(text), &levels);
        assert_eq!(runs.len(), 3, "{runs:?}");
        assert_eq!(&text[runs[0].range.clone()], "ab ");
        assert_eq!(&text[runs[1].range.clone()], "नमस्ते ");
        assert_eq!(&text[runs[2].range.clone()], "cd");
    }

    #[test]
    fn a_change_of_level_starts_a_run() {
        let text = "ab";
        let levels = vec![Level::ltr(), Level::rtl()];
        assert_eq!(runs(text, 0, &one_span(text), &levels).len(), 2);
    }

    #[test]
    fn neutral_text_has_no_script() {
        let text = "12 !";
        let levels = vec![Level::ltr(); text.len()];
        let runs = runs(text, 0, &one_span(text), &levels);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].script, None);
    }
}
