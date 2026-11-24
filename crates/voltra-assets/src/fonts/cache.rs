//! Layouts kept between frames: shaping every text twice a frame (once to
//! rasterize, once to draw) would be most of a frame's text cost.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use super::layout::{TextLayout, Wrap};
use super::span::Span;

/// How many layouts are kept before the cache starts over. Far more than a
/// screen of text; a game that lays out a new string every frame (a timer)
/// churns through it without growing.
const CAPACITY: usize = 4096;

/// What one layout was made from, and the layout.
#[derive(Debug)]
struct Entry {
    text: String,
    spans: Vec<Span>,
    wrap: Wrap,
    /// `None` is cached too: a text whose fonts have no face.
    layout: Option<Arc<TextLayout>>,
}

/// Layouts by what they were made from. Behind a mutex so that a layout
/// can be asked for through `&Fonts`, which the draw holds.
#[derive(Debug, Default)]
pub(crate) struct LayoutCache {
    entries: Mutex<HashMap<u64, Entry>>,
}

impl LayoutCache {
    /// The layout of `text` in `spans` at `wrap`, made by `make` the first
    /// time.
    pub fn get_or(
        &self,
        text: &str,
        spans: &[Span],
        wrap: Wrap,
        make: impl FnOnce() -> Option<TextLayout>,
    ) -> Option<Arc<TextLayout>> {
        let key = key(text, spans, wrap);
        if let Ok(entries) = self.entries.lock() {
            if let Some(entry) = entries.get(&key) {
                if entry.text == text && entry.spans == spans && entry.wrap == wrap {
                    return entry.layout.clone();
                }
            }
        }
        let layout = make().map(Arc::new);
        if let Ok(mut entries) = self.entries.lock() {
            if entries.len() >= CAPACITY {
                entries.clear();
            }
            entries.insert(
                key,
                Entry {
                    text: text.to_owned(),
                    spans: spans.to_vec(),
                    wrap,
                    layout: layout.clone(),
                },
            );
        }
        layout
    }

    /// Forgets every layout: a font was reloaded or the fallback chain
    /// changed, so glyph ids and advances may differ.
    pub fn clear(&mut self) {
        match self.entries.get_mut() {
            Ok(entries) => entries.clear(),
            // A panic while the lock was held left it poisoned; the map is
            // still a map, and emptying it is always safe.
            Err(poisoned) => poisoned.into_inner().clear(),
        }
    }

    /// How many layouts are kept.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.lock().map_or(0, |entries| entries.len())
    }
}

fn key(text: &str, spans: &[Span], wrap: Wrap) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    spans.hash(&mut hasher);
    wrap.hash(&mut hasher);
    hasher.finish()
}
