//! A run of frames boiled down to what a person or a tool reads: how long a
//! frame took, and where it went, scope by scope, as a tree.

use std::collections::HashMap;

use crate::recorder::Frame;

/// Milliseconds over a run of frames.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Stat {
    pub avg: f64,
    pub min: f64,
    pub max: f64,
}

/// One scope, by its place in the tree.
#[derive(Debug, Clone, PartialEq)]
pub struct ScopeStat {
    /// The names from the top down, `/`-joined: `frame/physics/solve`.
    pub path: String,
    pub name: &'static str,
    pub depth: u16,
    /// Times it ran per frame, on average over the run.
    pub calls: f64,
    /// Milliseconds per frame, all its calls together: average and worst.
    pub total: Stat,
    /// Average milliseconds per frame outside the scopes nested in it.
    pub own: f64,
}

/// A counter over a run of frames.
#[derive(Debug, Clone, PartialEq)]
pub struct CounterStat {
    pub name: &'static str,
    pub avg: f64,
    pub max: f64,
    pub last: f64,
}

/// What [`crate::summary`] answers.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Summary {
    pub frames: usize,
    /// Each frame, end to end.
    pub frame: Stat,
    /// In tree order: a scope, then what ran inside it.
    pub scopes: Vec<ScopeStat>,
    pub counters: Vec<CounterStat>,
}

#[derive(Default)]
struct Accumulator {
    name: &'static str,
    depth: u16,
    calls: u64,
    total: f64,
    max: f64,
    min: f64,
    children: f64,
    /// Frames it appeared in, so `min` is over those.
    seen: usize,
}

impl Summary {
    /// Summarises `frames`, whichever they are.
    pub fn of<'a>(frames: impl IntoIterator<Item = &'a Frame>) -> Self {
        let mut summary = Self::default();
        let mut order: Vec<String> = Vec::new();
        let mut scopes: HashMap<String, Accumulator> = HashMap::new();
        let mut counters: Vec<(&'static str, f64, f64, f64, usize)> = Vec::new();
        let mut frame_ms = Vec::new();

        for frame in frames {
            frame_ms.push(ms(frame.duration));
            // This frame's milliseconds per path, and per path its children's.
            let mut here: Vec<(String, f64, f64)> = Vec::new();
            for (path, name, depth, duration, parent) in tree(frame) {
                let entry = scopes.entry(path.clone()).or_insert_with(|| {
                    order.push(path.clone());
                    Accumulator {
                        name,
                        depth,
                        min: f64::INFINITY,
                        ..Accumulator::default()
                    }
                });
                entry.calls += 1;
                match here.iter_mut().find(|(p, _, _)| *p == path) {
                    Some(slot) => slot.1 += duration,
                    None => here.push((path.clone(), duration, 0.0)),
                }
                if let Some(parent) = parent {
                    match here.iter_mut().find(|(p, _, _)| *p == parent) {
                        Some(slot) => slot.2 += duration,
                        None => here.push((parent, 0.0, duration)),
                    }
                }
            }
            for (path, total, children) in here {
                if let Some(entry) = scopes.get_mut(&path) {
                    entry.total += total;
                    entry.children += children;
                    entry.max = entry.max.max(total);
                    entry.min = entry.min.min(total);
                    entry.seen += 1;
                }
            }
            for &(name, value) in &frame.counters {
                match counters.iter_mut().find(|c| c.0 == name) {
                    Some(c) => {
                        c.1 += value;
                        c.2 = c.2.max(value);
                        c.3 = value;
                        c.4 += 1;
                    }
                    None => counters.push((name, value, value, value, 1)),
                }
            }
        }

        let frames = frame_ms.len();
        summary.frames = frames;
        if frames == 0 {
            return summary;
        }
        let per_frame = |value: f64| value / frames as f64;
        summary.frame = Stat {
            avg: per_frame(frame_ms.iter().sum()),
            min: frame_ms.iter().copied().fold(f64::INFINITY, f64::min),
            max: frame_ms.iter().copied().fold(0.0, f64::max),
        };
        // Tree order: a scope right after its parent, siblings in the order
        // they first ran — not first appearance alone, which would put a
        // child that first ran in a later frame after unrelated scopes.
        let rank: HashMap<&str, usize> = order
            .iter()
            .enumerate()
            .map(|(i, path)| (path.as_str(), i))
            .collect();
        let key = |path: &str| -> Vec<usize> {
            path.match_indices('/')
                .map(|(i, _)| &path[..i])
                .chain(std::iter::once(path))
                .map(|prefix| rank.get(prefix).copied().unwrap_or(usize::MAX))
                .collect()
        };
        let mut sorted = order.clone();
        sorted.sort_by_cached_key(|path| key(path));
        summary.scopes = sorted
            .into_iter()
            .filter_map(|path| {
                let a = scopes.remove(&path)?;
                Some(ScopeStat {
                    path,
                    name: a.name,
                    depth: a.depth,
                    calls: per_frame(a.calls as f64),
                    total: Stat {
                        avg: per_frame(a.total),
                        // A frame it did not run in took it no time.
                        min: if a.seen < frames { 0.0 } else { a.min },
                        max: a.max,
                    },
                    own: per_frame((a.total - a.children).max(0.0)),
                })
            })
            .collect();
        summary.counters = counters
            .into_iter()
            .map(|(name, sum, max, last, seen)| CounterStat {
                name,
                avg: sum / seen as f64,
                max,
                last,
            })
            .collect();
        summary
    }
}

/// A frame's spans in the order they opened, each with its path, its
/// milliseconds and its parent's path.
fn tree(frame: &Frame) -> Vec<(String, &'static str, u16, f64, Option<String>)> {
    let mut spans: Vec<_> = frame.spans.iter().collect();
    // Opened first first; at the same instant, the outer one first.
    spans.sort_by(|a, b| a.start.cmp(&b.start).then(a.depth.cmp(&b.depth)));
    let mut open: Vec<(u16, String)> = Vec::new();
    let mut out = Vec::with_capacity(spans.len());
    for span in spans {
        while open.last().is_some_and(|(depth, _)| *depth >= span.depth) {
            open.pop();
        }
        let parent = open.last().map(|(_, path)| path.clone());
        let path = match &parent {
            Some(parent) => format!("{parent}/{}", span.name),
            None => span.name.to_owned(),
        };
        out.push((
            path.clone(),
            span.name,
            span.depth,
            ms(span.duration),
            parent,
        ));
        open.push((span.depth, path));
    }
    out
}

fn ms(duration: std::time::Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

#[cfg(test)]
mod tests;
