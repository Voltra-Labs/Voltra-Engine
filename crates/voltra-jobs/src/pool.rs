//! The worker threads and the queue they take jobs from.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::job::Job;
use crate::scope::{Scope, ScopeState};
use crate::task::{Slot, Task};

/// What the workers take jobs from: a scope's, which a waiting caller helps
/// with, and background tasks', which only workers run.
#[derive(Default)]
struct Queues {
    scoped: VecDeque<Job>,
    background: VecDeque<Job>,
}

/// What the workers and every scope share.
pub(crate) struct Shared {
    queue: Mutex<Queues>,
    /// Signalled when a job is queued, when a job finishes (a waiting scope
    /// may be done) and at shutdown.
    signal: Condvar,
    shutdown: AtomicBool,
}

impl Shared {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn push(&self, job: Job) {
        lock(&self.queue).scoped.push_back(job);
        self.signal.notify_one();
    }

    /// Queues a background task's job, for a worker only: a scope's caller
    /// helping with its own jobs must not pick up a long load and stall the
    /// frame it is part of.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn push_background(&self, job: Job) {
        lock(&self.queue).background.push_back(job);
        self.signal.notify_one();
    }

    /// Whether the pool has begun shutting down: no worker starts a
    /// background job after that, so a task still queued is run by whoever
    /// next asks for it.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn is_shut_down(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
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
            if let Some(job) = queue.scoped.pop_front() {
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
            queue: Mutex::new(Queues::default()),
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

    /// Hands `f` to the workers to run whenever one is free, and returns at
    /// once; [`Task::poll`] collects the result. A worker takes a scope's
    /// jobs before a task's, and a scope's caller never takes a task's, so
    /// a long load submitted here does not stall a frame's own jobs.
    ///
    /// Where there are no workers — a pool of zero, the web — the work runs
    /// on the first poll instead (see [`Task`]).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn submit<T: Send + 'static>(&self, f: impl FnOnce() -> T + Send + 'static) -> Task<T> {
        let slot = Arc::new(Slot::new(Box::new(f)));
        if self.workers.is_empty() {
            return Task::new(slot, None);
        }
        let job_slot = Arc::clone(&slot);
        self.shared
            .push_background(Job::owned(Box::new(move || job_slot.run())));
        Task::new(slot, Some(Arc::clone(&self.shared)))
    }

    /// Hands `f` over to run on the first [`Task::poll`]: the web has no
    /// workers.
    #[cfg(target_arch = "wasm32")]
    pub fn submit<T: 'static>(&self, f: impl FnOnce() -> T + 'static) -> Task<T> {
        Task::new(Arc::new(Slot::new(Box::new(f))))
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
                // A scope's jobs first: a frame is waiting on them, and a
                // load can wait one job longer.
                if let Some(job) = queue.scoped.pop_front() {
                    break Some(job);
                }
                // None after shutdown: a task left queued runs where it is
                // next polled, rather than holding the drop up.
                if shared.shutdown.load(Ordering::Acquire) {
                    break None;
                }
                if let Some(job) = queue.background.pop_front() {
                    break Some(job);
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
