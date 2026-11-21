//! The pool's promises: every job runs, borrows are safe, panics come back
//! to the caller, and nothing deadlocks.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use voltra_jobs::JobPool;

#[test]
fn every_job_runs_before_the_scope_returns() {
    let pool = JobPool::new(3);
    let count = AtomicUsize::new(0);
    pool.scope(|scope| {
        for _ in 0..1000 {
            scope.spawn(|| {
                count.fetch_add(1, Ordering::Relaxed);
            });
        }
    });
    assert_eq!(count.load(Ordering::Relaxed), 1000);
}

#[test]
fn jobs_write_through_borrows_of_the_caller_s_data() {
    let pool = JobPool::new(4);
    let mut squares = vec![0u64; 10_000];
    pool.for_each_chunk_mut(&mut squares, 64, |start, chunk| {
        for (i, slot) in chunk.iter_mut().enumerate() {
            let n = (start + i) as u64;
            *slot = n * n;
        }
    });
    assert!(squares
        .iter()
        .enumerate()
        .all(|(i, &s)| s == (i * i) as u64));
}

#[test]
fn a_pool_with_no_workers_runs_everything_on_the_caller() {
    let pool = JobPool::new(0);
    assert_eq!(pool.threads(), 1);
    let caller = std::thread::current().id();
    let seen = Mutex::new(Vec::new());
    pool.scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                seen.lock()
                    .expect("unpoisoned")
                    .push(std::thread::current().id())
            });
        }
    });
    let seen = seen.into_inner().expect("unpoisoned");
    assert_eq!(seen.len(), 8);
    assert!(seen.iter().all(|&id| id == caller));
}

#[test]
fn a_job_s_panic_reaches_the_caller_after_the_others_finish() {
    let pool = JobPool::new(2);
    let finished = AtomicUsize::new(0);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        pool.scope(|scope| {
            scope.spawn(|| panic!("boom"));
            for _ in 0..50 {
                scope.spawn(|| {
                    finished.fetch_add(1, Ordering::Relaxed);
                });
            }
        });
    }));
    let payload = result.expect_err("the panic came back");
    assert_eq!(payload.downcast_ref::<&str>(), Some(&"boom"));
    assert_eq!(finished.load(Ordering::Relaxed), 50);

    // And the pool still works.
    let after = AtomicUsize::new(0);
    pool.scope(|scope| {
        scope.spawn(|| {
            after.fetch_add(1, Ordering::Relaxed);
        })
    });
    assert_eq!(after.load(Ordering::Relaxed), 1);
}

#[test]
fn a_job_may_open_a_scope_of_its_own() {
    // One worker, so the nested scopes must be helped along by whoever
    // waits on them or nothing finishes.
    let pool = JobPool::new(1);
    let count = AtomicUsize::new(0);
    pool.scope(|outer| {
        for _ in 0..4 {
            outer.spawn(|| {
                pool.scope(|inner| {
                    for _ in 0..4 {
                        inner.spawn(|| {
                            count.fetch_add(1, Ordering::Relaxed);
                        });
                    }
                });
            });
        }
    });
    assert_eq!(count.load(Ordering::Relaxed), 16);
}

#[test]
fn a_scope_hands_back_its_closure_s_value() {
    let pool = JobPool::new(2);
    assert_eq!(pool.scope(|_| 42), 42);
    let mut empty: [u8; 0] = [];
    pool.for_each_chunk_mut(&mut empty, 1, |_, _| unreachable!("no chunks"));
}

#[test]
fn many_scopes_in_a_row_do_not_hang() {
    let pool = JobPool::with_available_parallelism();
    let mut total = 0usize;
    for round in 0..500 {
        let mut items = vec![1usize; round % 37 + 1];
        pool.for_each_chunk_mut(&mut items, 1, |_, chunk| {
            chunk.iter_mut().for_each(|v| *v *= 2)
        });
        total += items.iter().sum::<usize>();
    }
    assert_eq!(total, (0..500).map(|r| 2 * (r % 37 + 1)).sum());
}
