//! The calling thread's recorder, behind free functions: what the engine's
//! scopes and the loop's `end_frame` use.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::time::Instant;

use crate::recorder::{Frame, Recorder};
use crate::summary::Summary;

thread_local! {
    static RECORDER: RefCell<Recorder> = RefCell::new(Recorder::default());
}

/// A scope being timed; closes when dropped.
#[must_use = "a scope times until it is dropped: bind it, `let _scope = …`"]
pub struct Scope {
    open: Option<(&'static str, u16, Instant)>,
}

impl Drop for Scope {
    fn drop(&mut self) {
        if let Some((name, depth, start)) = self.open.take() {
            let now = Instant::now();
            RECORDER.with_borrow_mut(|recorder| recorder.close(name, depth, start, now));
        }
    }
}

/// Times `name` from now until the returned scope is dropped. Nothing but
/// a flag read while recording is off.
pub fn scope(name: &'static str) -> Scope {
    let depth = RECORDER.with_borrow_mut(Recorder::open);
    Scope {
        open: depth.map(|depth| (name, depth, Instant::now())),
    }
}

/// Sets the counter `name` to `value` for this frame.
pub fn count(name: &'static str, value: f64) {
    RECORDER.with_borrow_mut(|recorder| recorder.count(name, value));
}

/// Records a GPU pass timing, in milliseconds, that came back this frame.
pub fn gpu(name: &'static str, ms: f64) {
    RECORDER.with_borrow_mut(|recorder| recorder.gpu(name, ms));
}

/// Closes this thread's frame. Called once a frame by the loop that owns
/// the thread.
pub fn end_frame() {
    let now = Instant::now();
    RECORDER.with_borrow_mut(|recorder| recorder.end_frame(now));
}

pub fn set_enabled(enabled: bool) {
    let now = Instant::now();
    RECORDER.with_borrow_mut(|recorder| recorder.set_enabled(enabled, now));
}

pub fn is_enabled() -> bool {
    RECORDER.with_borrow(Recorder::is_enabled)
}

/// Forgets this thread's frames.
pub fn clear() {
    RECORDER.with_borrow_mut(Recorder::clear);
}

/// How many closed frames this thread holds.
pub fn frames_recorded() -> usize {
    RECORDER.with_borrow(|recorder| recorder.frames().len())
}

/// Hands `f` this thread's closed frames, oldest first.
pub fn with_frames<T>(f: impl FnOnce(&VecDeque<Frame>) -> T) -> T {
    RECORDER.with_borrow(|recorder| f(recorder.frames()))
}

/// The last `frames` closed frames of this thread, summarised.
pub fn summary(frames: usize) -> Summary {
    with_frames(|all| {
        let skip = all.len().saturating_sub(frames);
        Summary::of(all.iter().skip(skip))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_on_this_thread_land_in_its_frames_and_not_another_threads() {
        set_enabled(true);
        {
            let _outer = scope("outer");
            let _inner = scope("inner");
        }
        end_frame();
        assert_eq!(frames_recorded(), 1);
        let names =
            with_frames(|frames| frames[0].spans.iter().map(|s| s.name).collect::<Vec<_>>());
        assert_eq!(names, ["inner", "outer"]);

        let elsewhere = std::thread::spawn(|| {
            let _scope = scope("not recorded: off on this thread");
            end_frame();
            frames_recorded()
        })
        .join()
        .expect("the thread ran");
        assert_eq!(elsewhere, 0);
        set_enabled(false);
        clear();
    }
}
