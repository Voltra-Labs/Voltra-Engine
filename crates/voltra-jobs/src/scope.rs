//! Spawning jobs that borrow from the caller of [`JobPool::scope`].
//!
//! [`JobPool::scope`]: crate::JobPool::scope

use std::any::Any;
use std::marker::PhantomData;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::job::Job;
use crate::pool::Shared;

/// What a scope's jobs report back: how many are still out, and the first
/// panic among them.
#[derive(Default)]
pub(crate) struct ScopeState {
    pending: AtomicUsize,
    panic: Mutex<Option<Box<dyn Any + Send>>>,
}

impl ScopeState {
    pub(crate) fn is_done(&self) -> bool {
        self.pending.load(Ordering::Acquire) == 0
    }

    /// Resumes the first panic a job raised, if one did.
    pub(crate) fn resume_panic(&self) {
        let payload = self
            .panic
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(payload) = payload {
            panic::resume_unwind(payload);
        }
    }
}

/// Spawns jobs that may borrow anything living for `'env`.
///
/// `'scope` is the scope's own life, shorter than `'env`; both are
/// invariant, so neither can be stretched to let a job keep a borrow past
/// the scope.
pub struct Scope<'scope, 'env: 'scope> {
    shared: &'scope Shared,
    state: &'scope ScopeState,
    _scope: PhantomData<&'scope mut &'scope ()>,
    _env: PhantomData<&'env mut &'env ()>,
}

impl<'scope, 'env> Scope<'scope, 'env> {
    pub(crate) fn new(shared: &'scope Shared, state: &'scope ScopeState) -> Self {
        Self {
            shared,
            state,
            _scope: PhantomData,
            _env: PhantomData,
        }
    }

    /// Queues `f` to run on the pool before the scope returns.
    pub fn spawn(&self, f: impl FnOnce() + Send + 'env) {
        self.state.pending.fetch_add(1, Ordering::AcqRel);
        // The state is the scope's, which waits for this job; erasing its
        // lifetime with the job's is covered by the same argument below.
        let state: &'env ScopeState = unsafe { &*(self.state as *const ScopeState) };
        let shared: &'env Shared = unsafe { &*(self.shared as *const Shared) };
        let job: Box<dyn FnOnce() + Send + 'env> = Box::new(move || {
            if let Err(payload) = panic::catch_unwind(AssertUnwindSafe(f)) {
                let mut first = state
                    .panic
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                first.get_or_insert(payload);
            }
            state.pending.fetch_sub(1, Ordering::AcqRel);
            shared.finished();
        });
        // SAFETY: `JobPool::scope` does not return — normally or by
        // unwinding — until `pending` is back to zero, which this job's last
        // act is to make it; after that it touches nothing it borrowed but
        // `shared`, which the pool owns and outlives the scope. So every
        // `'env` borrow outlives every use of it.
        let job = unsafe { Job::erase(job) };
        self.shared.push(job);
    }
}
