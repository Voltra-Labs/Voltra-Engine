//! Background tasks: finished by a worker while the caller carries on, run
//! on the poll where no worker will, cancelled by a drop, and never picked
//! up by a scope's waiting caller.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

use voltra_jobs::JobPool;

/// Polls `task` until it answers, failing after a few seconds.
fn poll_until<T>(task: &mut voltra_jobs::Task<T>) -> T {
    let start = Instant::now();
    loop {
        if let Some(value) = task.poll() {
            return value;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "never finished");
        std::thread::yield_now();
    }
}

#[test]
fn a_worker_finishes_a_task_while_the_caller_carries_on() {
    let pool = JobPool::new(2);
    let gate = Arc::new(Barrier::new(2));
    let held = gate.clone();
    let mut task = pool.submit(move || {
        held.wait();
        6 * 7
    });
    // Not done until the caller lets it: a poll never blocks.
    assert_eq!(task.poll(), None);
    gate.wait();
    assert_eq!(poll_until(&mut task), 42);
    // Once only.
    assert_eq!(task.poll(), None);
}

#[test]
fn with_no_workers_the_first_poll_runs_it() {
    let pool = JobPool::new(0);
    let ran = Arc::new(AtomicBool::new(false));
    let flag = ran.clone();
    let mut task = pool.submit(move || {
        flag.store(true, Ordering::SeqCst);
        "done"
    });
    assert!(!ran.load(Ordering::SeqCst), "nothing runs before the poll");
    assert_eq!(task.poll(), Some("done"));
}

#[test]
fn a_task_dropped_before_it_starts_never_runs() {
    let pool = JobPool::new(0);
    let ran = Arc::new(AtomicBool::new(false));
    let flag = ran.clone();
    let task = pool.submit(move || flag.store(true, Ordering::SeqCst));
    drop(task);
    drop(pool);
    assert!(!ran.load(Ordering::SeqCst));
}

#[test]
fn wait_blocks_for_the_result() {
    let pool = JobPool::new(1);
    let task = pool.submit(|| (1..=10).sum::<u32>());
    assert_eq!(task.wait(), Some(55));
}

#[test]
fn a_panic_in_a_task_comes_back_on_the_poll() {
    let pool = JobPool::new(1);
    let task = pool.submit(|| -> u32 { panic!("bad file") });
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| task.wait()));
    assert!(caught.is_err());
    // The worker survived it.
    assert_eq!(pool.submit(|| 3).wait(), Some(3));
}

#[test]
fn a_scope_s_caller_does_not_pick_up_a_background_task() {
    // One worker, held busy by a task until the scope is over: if the
    // scope's caller took background work while it waited, the second
    // task would run on it and finish before the gate opens.
    let pool = JobPool::new(1);
    let gate = Arc::new(Barrier::new(2));
    let held = gate.clone();
    let mut busy = pool.submit(move || {
        held.wait();
    });
    let ran_second = Arc::new(AtomicBool::new(false));
    let flag = ran_second.clone();
    let mut second = pool.submit(move || flag.store(true, Ordering::SeqCst));
    let jobs = AtomicUsize::new(0);
    pool.scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                jobs.fetch_add(1, Ordering::Relaxed);
            });
        }
    });
    assert_eq!(jobs.load(Ordering::Relaxed), 8);
    assert!(!ran_second.load(Ordering::SeqCst));
    gate.wait();
    poll_until(&mut busy);
    poll_until(&mut second);
    assert!(ran_second.load(Ordering::SeqCst));
}

#[test]
fn a_task_left_queued_when_the_pool_goes_runs_on_its_poll() {
    let pool = JobPool::new(1);
    let gate = Arc::new(Barrier::new(2));
    let held = gate.clone();
    let mut busy = pool.submit(move || held.wait());
    let mut queued = pool.submit(|| 9);
    // Let the worker finish the first, then shut down: the second may or
    // may not have started, and either way a poll gets it.
    gate.wait();
    poll_until(&mut busy);
    drop(pool);
    assert_eq!(poll_until(&mut queued), 9);
}
