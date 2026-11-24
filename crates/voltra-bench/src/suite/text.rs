//! Text layout: a string into kerned, wrapped glyph positions.

use voltra_assets::fonts::Wrap;
use voltra_assets::Fonts;

use super::{assets, Bench, Outcome};
use crate::harness::{iter, measure};

/// Pixels per em: body text.
const PX: u32 = 16;

/// About a thousand characters of prose, wrapped: a dialogue box, a
/// tooltip's worth of help, a credits page's paragraph.
const PARAGRAPH: &str = "The engine draws a world in painter's order: whatever is written \
last covers what came before, and the batch keeps every sprite where its sorting layer and \
order put it. A tilemap layer is quads in the same batch, cut into chunks the camera culls, \
and kept from one frame to the next while its cells stay as they were. Physics steps at a \
fixed rate, its contacts warm-started from the step before, and bodies at rest fall asleep \
by island until something wakes them. Text is laid out on baselines, kerned pair by pair, \
and wrapped greedily between words, as Godot and Unity wrap theirs; a word wider than the \
line is broken between characters rather than left sticking out. The interface is rects \
anchored to their parents and stacks that place their children in a row or a column, with \
scrolls that clip what passes their edges. Scenes are text in a pull request, merged in git \
and loaded all at once, every entity or none. What a frame costs is measured here, so that \
what the next change costs is a number rather than a guess.";

pub(super) fn benches() -> Vec<Bench> {
    vec![
        Bench::new("text/layout/paragraph-1k", 15.0, |config| {
            with_font(|fonts| {
                let font = fonts.try_get(fonts.default_font())?;
                let wrap = Wrap {
                    max_width: Some(400.0),
                    line_spacing: 1.0,
                };
                measure(config, 1, iter(|| font.layout(PARAGRAPH, PX, wrap)))
            })
        }),
        Bench::new("text/layout/labels-100", 25.0, |config| {
            let labels: Vec<String> = (0..100)
                .map(|i| format!("Score {}  ·  Coins {}", i * 250, i % 37))
                .collect();
            with_font(|fonts| {
                let font = fonts.try_get(fonts.default_font())?;
                measure(
                    config,
                    1,
                    iter(|| {
                        labels
                            .iter()
                            .filter_map(|label| font.layout(label, PX, Wrap::default()))
                            .map(|layout| layout.glyphs.len())
                            .sum::<usize>()
                    }),
                )
            })
        }),
    ]
}

/// Runs `time` against the shipped default font, or skips when it will not
/// load — `time` answering `None` is a face with nothing to lay out.
fn with_font(time: impl FnOnce(&Fonts) -> Option<crate::stats::Stats>) -> Outcome {
    let fonts = Fonts::new(assets());
    match time(&fonts) {
        Some(stats) => Outcome::Measured(stats),
        None => Outcome::Skipped(format!(
            "the default font under {} would not load or lay out",
            assets().display()
        )),
    }
}
