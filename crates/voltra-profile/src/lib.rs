//! Where a frame's time goes: named scopes, nested, and counters, recorded
//! frame by frame and summarised on request.
//!
//! ```ignore
//! let _scope = voltra_profile::scope("physics");   // timed until dropped
//! voltra_profile::count("entities", 1200.0);
//! voltra_profile::end_frame();                     // once a frame, by the loop
//! let summary = voltra_profile::summary(120);      // the last 120 frames
//! ```
//!
//! Unity's `ProfilerMarker.Auto()` for the scopes and Godot's monitors for
//! the counters. **Off until switched on** ([`set_enabled`]): a disabled
//! scope is a thread-local flag read and nothing else, so the engine can
//! leave its scopes in place.
//!
//! **Per thread.** Each thread records into its own [`Recorder`] and
//! closes its own frames: an `App` lives on one thread, and two of them —
//! two tests, two headless runs — never see each other's numbers, with no
//! lock on the way. A scope on a worker thread is recorded there and read
//! by nobody until that thread reads it.

#![warn(missing_docs)]

mod clock;
mod recorder;
mod summary;
mod thread;

pub use recorder::{Frame, Recorder, Span, HISTORY};
pub use summary::{CounterStat, GpuStat, ScopeStat, Stat, Summary};
pub use thread::{
    clear, count, end_frame, frames_recorded, gpu, is_enabled, scope, set_enabled, summary,
    with_frames, Scope,
};
