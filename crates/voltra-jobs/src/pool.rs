//! The worker threads and the queue they take jobs from.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::job::Job;
use crate::scope::{Scope, ScopeState};

/// What the workers and every scope share.
pub(crate) struct Shared {
    queue: Mutex<VecDeque<Job>>,
    /// Signalled when a job is queued, when a job finishes (a waiting scope
    /// may be done) and at shutdown.
    signal: Condvar,
    shutdown: AtomicBool,
}

impl Shared {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn push(&self, job: Job) {
        lock(&self.queue).push_back(job);
        self.signal.notify_one();
    }

    /// Wakes everyone waiting: a job finished, and the scope it belonged to
    /// may be the one a waiter is waiting for.
    pub(crate) fn finished(&self) {
        let _queue = lock(&self.queue);
        self.signal.notify_all();
    }

    /// Runs queued jobs until `done` holds, sleeping when there are none.
    pub(crate) fn help_until(&self, done: impl Fn() -> bool) {
        loop {
            if done() {
                return;
            }
            let mut queue = lock(&self.queue);
            if let Some(job) = queue.pop_front() {
                drop(queue);
                job.run();
                continue;
            }
            if done() {
                return;
            }
            // Woken by `finished`; the timeout only guards a wakeup missed
            // between the check above and the wait.
            let (guard, _) = self
                .signal
                .wait_timeout(queue, Duration::from_millis(1))
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            drop(guard);
        }
    }
}

/// A fixed set of worker threads, made once and joined on drop.
pub struct JobPool {
    shared: Arc<Shared>,
    workers: Vec<JoinHandle<()>>,
}

impl JobPool {
    /// A pool of `workers` threads beside the caller. Zero is a pool that
    /// runs every job on the thread that waits for it — what a machine with
    /// one core, or a test that wants order, gets.
    pub fn new(workers: usize) -> Self {
        let shared = Arc::new(Shared {
            queue: Mutex::new(VecDeque::new()),
            signal: Condvar::new(),
            shutdown: AtomicBool::new(false),
        });
        let mut spawned = Vec::with_capacity(workers);
        for index in 0..workers {
            let shared = Arc::clone(&shared);
            let spawn = std::thread::Builder::new()
                .name(format!("voltra-job-{index}"))
                .spawn(move || work(&shared));
            match spawn {
                Ok(handle) => spawned.push(handle),
                // Fewer workers is slower, not wrong: the caller still runs
                // whatever is queued.
                Err(e) => log::warn!("job worker {index} not started: {e}"),
            }
        }
        Self {
            shared,
            workers: spawned,
        }
    }

    /// One worker per core but the caller's.
    pub fn with_available_parallelism() -> Self {
        let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
        Self::new(cores.saturating_sub(1))
    }

    /// How many jobs can run at once: the workers and the caller.
    pub fn threads(&self) -> usize {
        self.workers.len() + 1
    }

    /// Runs `f`, which may spawn jobs borrowing anything that outlives this
    /// call, and returns once every one of them has finished.
    ///
    /// If a job panics, the others still run to the end and the first panic
    /// is resumed here, after them.
    pub fn scope<'env, R>(&self, f: impl FnOnce(&Scope<'_, 'env>) -> R) -> R {
        let state = ScopeState::default();
        let scope = Scope::new(&self.shared, &state);
        // Waits on the way out even when `f` unwinds, so no job outlives the
        // borrows it was spawned with.
        let wait = WaitOnDrop {
            shared: &self.shared,
            state: &state,
        };
        let result = f(&scope);
        drop(wait);
        state.resume_panic();
        result
    }

    /// Calls `f` on consecutive chunks of `items`, each at least `min_chunk`
    /// long, across the pool. `f` is told where its chunk starts.
    pub fn for_each_chunk_mut<T: crate::MaybeSend>(
        &self,
        items: &mut [T],
        min_chunk: usize,
        f: impl Fn(usize, &mut [T]) + crate::MaybeSync,
    ) {
        if items.is_empty() {
            return;
        }
        let chunk = items.len().div_ceil(self.threads()).max(min_chunk.max(1));
        let f = &f;
        self.scope(|scope| {
            for (index, part) in items.chunks_mut(chunk).enumerate() {
                scope.spawn(move || f(index * chunk, part));
            }
        });
    }
}

impl Default for JobPool {
    fn default() -> Self {
        Self::with_available_parallelism()
    }
}

impl std::fmt::Debug for JobPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobPool")
            .field("threads", &self.threads())
            .finish()
    }
}

impl Drop for JobPool {
    fn drop(&mut self) {
        self.shared.shutdown.store(true, Ordering::Release);
        self.shared.finished();
        for worker in self.workers.drain(..) {
            if worker.join().is_err() {
                log::error!("a job worker panicked outside a job");
            }
        }
    }
}

struct WaitOnDrop<'a> {
    shared: &'a Shared,
    state: &'a ScopeState,
}

impl Drop for WaitOnDrop<'_> {
    fn drop(&mut self) {
        self.shared.help_until(|| self.state.is_done());
    }
}

fn work(shared: &Shared) {
    loop {
        let job = {
            let mut queue = lock(&shared.queue);
            loop {
                if let Some(job) = queue.pop_front() {
                    break Some(job);
                }
                if shared.shutdown.load(Ordering::Acquire) {
                    break None;
                }
                queue = shared
                    .signal
                    .wait(queue)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
        };
        match job {
            Some(job) => job.run(),
            None => return,
        }
    }
}

/// The queue's lock. A job's panic is caught inside the job, so nothing
/// holding the lock panics; poisoning is recovered from rather than spread.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
