use std::time::Duration;

use super::*;
use crate::recorder::Span;

fn span(name: &'static str, depth: u16, start: u64, ms: u64) -> Span {
    Span {
        name,
        depth,
        start: Duration::from_millis(start),
        duration: Duration::from_millis(ms),
    }
}

fn frame(duration: u64, spans: Vec<Span>, counters: Vec<(&'static str, f64)>) -> Frame {
    Frame {
        index: 0,
        duration: Duration::from_millis(duration),
        spans,
        counters,
    }
}

#[test]
fn scopes_become_a_tree_with_own_time_and_calls_per_frame() {
    let frames = [
        frame(
            16,
            vec![
                span("broad", 1, 1, 2),
                span("solve", 1, 3, 4),
                span("solve", 1, 7, 2),
                span("physics", 0, 0, 10),
                span("draw", 0, 10, 4),
            ],
            vec![("entities", 10.0)],
        ),
        frame(20, vec![span("physics", 0, 0, 6)], vec![("entities", 20.0)]),
    ];
    let summary = Summary::of(&frames);
    assert_eq!(summary.frames, 2);
    assert!((summary.frame.avg - 18.0).abs() < 1e-9);
    assert!((summary.frame.max - 20.0).abs() < 1e-9);

    let paths: Vec<&str> = summary.scopes.iter().map(|s| s.path.as_str()).collect();
    assert_eq!(paths, ["physics", "physics/broad", "physics/solve", "draw"]);
    let physics = &summary.scopes[0];
    assert!((physics.total.avg - 8.0).abs() < 1e-9, "(10 + 6) / 2");
    assert!((physics.total.max - 10.0).abs() < 1e-9);
    assert!((physics.own - 4.0).abs() < 1e-9, "(10 - 8 + 6) / 2");
    let solve = &summary.scopes[2];
    assert!(
        (solve.calls - 1.0).abs() < 1e-9,
        "two calls over two frames"
    );
    assert!(
        (solve.total.max - 6.0).abs() < 1e-9,
        "both calls in one frame"
    );
    assert_eq!(solve.total.min, 0.0, "absent from a frame");

    let entities = &summary.counters[0];
    assert_eq!(
        (entities.avg, entities.max, entities.last),
        (15.0, 20.0, 20.0)
    );
}

#[test]
fn no_frames_is_an_empty_summary() {
    assert_eq!(Summary::of(&[]), Summary::default());
}

#[test]
fn a_child_first_seen_in_a_later_frame_still_sits_under_its_parent() {
    let frames = [
        frame(
            10,
            vec![span("physics", 0, 0, 1), span("draw", 0, 1, 1)],
            vec![],
        ),
        frame(
            10,
            vec![
                span("step", 1, 0, 1),
                span("physics", 0, 0, 2),
                span("draw", 0, 2, 1),
            ],
            vec![],
        ),
    ];
    let paths: Vec<String> = Summary::of(&frames)
        .scopes
        .into_iter()
        .map(|s| s.path)
        .collect();
    assert_eq!(paths, ["physics", "physics/step", "draw"]);
}
