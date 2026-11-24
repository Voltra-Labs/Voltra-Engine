//! Work handed to the pool to finish whenever it can, its result collected
//! by polling: what a load that spans frames needs, where a scope would
//! hold the frame until it was done.
//!
//! Godot's `WorkerThreadPool::add_task` with `is_task_completed`, and
//! Bevy's `AsyncComputeTaskPool::spawn` polled with `block_on(poll_once)`:
//! the frame asks each frame whether the work is done and carries on when
//! it is not. A task owns what it works on (`'static`), since nothing
//! waits for it to give a borrow back.
//!
//! **Where no worker will run it** — on the web, on a pool of zero
//! workers, on a pool shutting down — the first [`Task::poll`] runs it
//! there and then. A caller that polls only while its frame budget lasts
//! is therefore spending that budget on the task, which is how a loader
//! degrades to the main thread without a second code path.

use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread;

#[cfg(not(target_arch = "wasm32"))]
use crate::pool::Shared;

/// The work itself, `Send` where it crosses to a worker.
#[cfg(not(target_arch = "wasm32"))]
type Work<T> = Box<dyn FnOnce() -> T + Send>;
#[cfg(target_arch = "wasm32")]
type Work<T> = Box<dyn FnOnce() -> T>;

enum State<T> {
    Queued(Work<T>),
    Running,
    Done(thread::Result<T>),
    /// Its result collected, or the task dropped before anything ran it.
    Gone,
}

/// What a task and the job running it share.
pub(crate) struct Slot<T> {
    state: Mutex<State<T>>,
    done: Condvar,
}

impl<T> Slot<T> {
    pub(crate) fn new(work: Work<T>) -> Self {
        Self {
            state: Mutex::new(State::Queued(work)),
            done: Condvar::new(),
        }
    }

    /// Runs the work unless something already started it (or it was
    /// cancelled): what a worker does, and what a poll does where no
    /// worker will.
    pub(crate) fn run(&self) {
        let work = {
            let mut state = lock(&self.state);
            match std::mem::replace(&mut *state, State::Running) {
                State::Queued(work) => work,
                other => {
                    *state = other;
                    return;
                }
            }
        };
        let result = panic::catch_unwind(AssertUnwindSafe(work));
        *lock(&self.state) = State::Done(result);
        self.done.notify_all();
    }
}

/// The result, later, of work handed to [`JobPool::submit`].
///
/// Dropping a task that has not started cancels it: the work never runs.
/// One already running finishes on its worker and its result is dropped.
///
/// [`JobPool::submit`]: crate::JobPool::submit
pub struct Task<T> {
    slot: Arc<Slot<T>>,
    /// The pool whose workers will run it; `None` when it has none.
    #[cfg(not(target_arch = "wasm32"))]
    pool: Option<Arc<Shared>>,
}

impl<T> Task<T> {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn new(slot: Arc<Slot<T>>, pool: Option<Arc<Shared>>) -> Self {
        Self { slot, pool }
    }

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn new(slot: Arc<Slot<T>>) -> Self {
        Self { slot }
    }

    /// Whether no worker will ever run this task, so a poll must.
    fn runs_here(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.pool.as_ref().is_none_or(|pool| pool.is_shut_down())
        }
        #[cfg(target_arch = "wasm32")]
        {
            true
        }
    }

    /// The result, once: `Some` the first poll after the work finished,
    /// `None` before that and after it. Where no worker will run the work
    /// (see the module), this runs it first.
    ///
    /// A task whose work panicked resumes the panic here.
    pub fn poll(&mut self) -> Option<T> {
        if self.runs_here() {
            self.slot.run();
        }
        self.take()
    }

    /// Whether the work has finished and its result is waiting.
    pub fn is_finished(&self) -> bool {
        matches!(*lock(&self.slot.state), State::Done(_))
    }

    /// Blocks until the result is in, running the work here if no worker
    /// has started it. `None` when [`poll`](Self::poll) already took it.
    pub fn wait(mut self) -> Option<T> {
        self.slot.run();
        let mut state = lock(&self.slot.state);
        while matches!(*state, State::Running) {
            state = self
                .slot
                .done
                .wait(state)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        drop(state);
        self.take()
    }

    fn take(&mut self) -> Option<T> {
        let mut state = lock(&self.slot.state);
        if !matches!(*state, State::Done(_)) {
            return None;
        }
        match std::mem::replace(&mut *state, State::Gone) {
            State::Done(Ok(value)) => Some(value),
            State::Done(Err(payload)) => {
                drop(state);
                panic::resume_unwind(payload)
            }
            _ => None,
        }
    }
}

impl<T> Drop for Task<T> {
    fn drop(&mut self) {
        let mut state = lock(&self.slot.state);
        if matches!(*state, State::Queued(_)) {
            *state = State::Gone;
        }
    }
}

impl<T> std::fmt::Debug for Task<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Task")
            .field("finished", &self.is_finished())
            .finish()
    }
}

/// The slot's lock. The work runs outside it and a panic in the work is
/// caught, so poisoning is recovered from rather than spread.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
