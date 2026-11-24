//! Where a paragraph's lines end: at UAX #14 break opportunities, greedily.

use std::ops::Range;

use unicode_linebreak::BreakOpportunity;

use super::shape::Shaped;

/// Glyphs that move together: one cluster of the shaper's output, which a
/// line never splits.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Cluster {
    /// Its bytes in the whole string: from its first character to the next
    /// cluster's.
    pub bytes: Range<usize>,
    /// Its glyphs, as indices into the paragraph's shaped glyphs.
    pub glyphs: Range<usize>,
    /// The sum of its glyphs' advances, in pixels.
    pub width: f32,
    /// Whether it is only white space: it hangs past a line's end rather
    /// than pushing a word onto the next one.
    pub space: bool,
}

/// `shaped` (logical order) grouped into clusters; `end` is the
/// paragraph's last byte, exclusive.
pub(crate) fn clusters(text: &str, shaped: &[Shaped], end: usize) -> Vec<Cluster> {
    let mut out: Vec<Cluster> = Vec::new();
    for (i, glyph) in shaped.iter().enumerate() {
        match out.last_mut() {
            Some(last) if last.bytes.start == glyph.cluster => {
                last.glyphs.end = i + 1;
                last.width += glyph.advance;
            }
            _ => out.push(Cluster {
                bytes: glyph.cluster..glyph.cluster,
                glyphs: i..i + 1,
                width: glyph.advance,
                space: false,
            }),
        }
    }
    let starts: Vec<usize> = out.iter().map(|c| c.bytes.start).collect();
    for (i, cluster) in out.iter_mut().enumerate() {
        cluster.bytes.end = starts
            .get(i + 1)
            .copied()
            .unwrap_or(end)
            .max(cluster.bytes.start);
        let chars = text.get(cluster.bytes.clone()).unwrap_or("");
        cluster.space =
            !chars.is_empty() && chars.chars().all(|c| c.is_whitespace() || c.is_control());
    }
    out
}

/// The clusters of each line of `paragraph` (bytes `base..` of the string),
/// as ranges of `clusters`. Always at least one line — empty for an empty
/// paragraph.
///
/// A line ends where UAX #14 allows (after a space, between two CJK
/// characters, after a hyphen) or must (a line separator). Wrapping is
/// greedy, as Godot's and Unity's is: as many clusters as fit, the break at
/// the last opportunity before the one that does not; white space at the
/// break hangs and is dropped. A word wider than the whole line breaks
/// between clusters rather than sticking out. With no `max_width` only the
/// mandatory breaks apply and nothing is dropped.
pub(crate) fn lines(
    paragraph: &str,
    base: usize,
    clusters: &[Cluster],
    max_width: Option<f32>,
) -> Vec<Range<usize>> {
    let mut allowed = Vec::new();
    let mut mandatory = Vec::new();
    for (at, kind) in unicode_linebreak::linebreaks(paragraph) {
        // The end of the text is always a mandatory break; it is not one
        // inside the paragraph.
        if at >= paragraph.len() {
            continue;
        }
        match kind {
            BreakOpportunity::Allowed => allowed.push(base + at),
            BreakOpportunity::Mandatory => mandatory.push(base + at),
        }
    }
    let may = |i: usize| allowed.binary_search(&clusters[i].bytes.start).is_ok();
    let must = |i: usize| mandatory.binary_search(&clusters[i].bytes.start).is_ok();

    let mut out = Vec::new();
    let mut start = 0;
    loop {
        let mut width = 0.0;
        let mut last_break = None;
        let mut forced = false;
        let mut i = start;
        while i < clusters.len() {
            let cluster = &clusters[i];
            if i > start && must(i) {
                forced = true;
                break;
            }
            if i > start && may(i) {
                last_break = Some(i);
            }
            // Spaces never break the width: they hang at a break.
            if let Some(max) = max_width {
                if !cluster.space && i > start && width + cluster.width > max {
                    break;
                }
            }
            width += cluster.width;
            i += 1;
        }
        if i == clusters.len() {
            out.push(start..trim(clusters, start, i, max_width.is_some()));
            return out;
        }
        if forced {
            out.push(start..trim(clusters, start, i, max_width.is_some()));
            start = i;
            continue;
        }
        let end = match last_break {
            Some(at) if at > start => at,
            _ => i,
        };
        out.push(start..trim(clusters, start, end, true));
        start = end;
        while start < clusters.len() && clusters[start].space {
            start += 1;
        }
        if start == clusters.len() {
            return out;
        }
    }
}

/// `end` moved back over trailing white space when `hang`, but not past
/// `start`.
fn trim(clusters: &[Cluster], start: usize, mut end: usize, hang: bool) -> usize {
    while hang && end > start && clusters[end - 1].space {
        end -= 1;
    }
    end
}
