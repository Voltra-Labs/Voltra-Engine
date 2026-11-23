//! A profile summary as the bridge and the MCP tools answer it.

use serde_json::{json, Value};
use voltra_profile::{Stat, Summary};

/// `summary` as JSON: the frame's time, each scope by its path with its
/// calls and milliseconds, the counters. Rounded to microseconds — finer is
/// noise, and every digit is a token to whoever reads it.
pub fn to_json(summary: &Summary) -> Value {
    let fps = if summary.frame.avg > 0.0 {
        1000.0 / summary.frame.avg
    } else {
        0.0
    };
    json!({
        "frames": summary.frames,
        "frame_ms": stat(summary.frame),
        "fps": round(fps),
        "scopes": summary.scopes.iter().map(|scope| json!({
            "path": scope.path,
            "calls": round(scope.calls),
            "avg_ms": round(scope.total.avg),
            "max_ms": round(scope.total.max),
            "own_ms": round(scope.own),
            "share": if summary.frame.avg > 0.0 { round(scope.total.avg / summary.frame.avg) } else { 0.0 },
        })).collect::<Vec<_>>(),
        // Per run of each pass on the GPU; timings arrive frames late, so
        // `samples` need not equal `frames`. Empty where the adapter cannot
        // time passes, or in a headless run, which draws nothing.
        "gpu": summary.gpu.iter().map(|pass| json!({
            "pass": pass.name,
            "avg_ms": round(pass.avg),
            "max_ms": round(pass.max),
            "samples": pass.samples,
        })).collect::<Vec<_>>(),
        "counters": summary.counters.iter().map(|counter| json!({
            "name": counter.name,
            "avg": round(counter.avg),
            "max": round(counter.max),
            "last": round(counter.last),
        })).collect::<Vec<_>>(),
    })
}

fn stat(stat: Stat) -> Value {
    json!({ "avg": round(stat.avg), "min": round(stat.min), "max": round(stat.max) })
}

fn round(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_summary_reads_as_no_frames() {
        let value = to_json(&Summary::default());
        assert_eq!(value["frames"], 0);
        assert_eq!(value["scopes"], json!([]));
    }
}
