//! A pool of worker threads that run jobs borrowing from their caller.
//!
//! The shape of Bevy's `ComputeTaskPool::scope` and Godot's
//! `WorkerThreadPool` group tasks, kept to what a frame needs: a fixed set
//! of threads made once, and a [`JobPool::scope`] that spawns jobs which may
//! borrow anything outliving the call, returning only once every job is
//! done. `std::thread::scope` has the same borrowing rule but makes a thread
//! per job — tens of microseconds each, every frame.
//!
//! The calling thread works too: while it waits for its scope it runs
//! queued jobs, so a pool of `n` workers runs `n + 1` jobs at once, and a
//! job that opens a scope of its own cannot deadlock the pool.
//!
//! Work that spans frames — a region of a big world read and parsed while
//! the game plays — is [`JobPool::submit`]ted instead: it returns a
//! [`Task`] at once, polled each frame for its result, and only workers
//! run it, so no frame's own scope waits behind it.
//!
//! **On the web** (`wasm32`) there are no threads: the pool has no workers
//! and a job runs the moment it is spawned, so the bounds are
//! [`MaybeSend`]/[`MaybeSync`] — `Send`/`Sync` natively, nothing there.
//!
//! A job that panics does not take a worker with it: the panic is caught,
//! the scope finishes its other jobs, and the first panic is resumed on the
//! caller.
//!
//! ```
//! let pool = voltra_jobs::JobPool::new(2);
//! let mut doubled = vec![1, 2, 3, 4];
//! pool.for_each_chunk_mut(&mut doubled, 1, |_, chunk| {
//!     chunk.iter_mut().for_each(|n| *n *= 2);
//! });
//! assert_eq!(doubled, [2, 4, 6, 8]);
//! ```
//!
//! A job cannot borrow something that dies before its scope does:
//!
//! ```compile_fail
//! let pool = voltra_jobs::JobPool::new(1);
//! pool.scope(|scope| {
//!     let local = 5;
//!     scope.spawn(|| {
//!         let _ = &local;
//!     });
//! });
//! ```

mod job;
mod maybe;
mod pool;
mod scope;
mod task;

pub use maybe::{MaybeSend, MaybeSync};
pub use pool::JobPool;
pub use scope::Scope;
pub use task::Task;
