//! Devices opened from several threads at once, as a test binary's parallel
//! tests do.
//!
//! With every backend enabled, NVIDIA's Vulkan and OpenGL drivers on Windows
//! can deadlock when two threads initialise them together: a parallel test
//! run hung about one time in five. Opening is serialised inside
//! `voltra-render`; this test fails on a deadline instead of hanging if that
//! ever stops being true.

use std::sync::mpsc;
use std::time::Duration;

use voltra_render::GpuContext;

const THREADS: usize = 8;
const ROUNDS: usize = 4;
const DEADLINE: Duration = Duration::from_secs(60);

#[test]
fn contexts_opened_from_many_threads_at_once_all_open() {
    let (done, finished) = mpsc::channel();
    for _ in 0..THREADS {
        let done = done.clone();
        std::thread::spawn(move || {
            for _ in 0..ROUNDS {
                // `None` (no adapter) is fine: the point is not hanging.
                drop(GpuContext::headless(16, 16));
            }
            let _ = done.send(());
        });
    }
    drop(done);
    for opened in 0..THREADS {
        assert!(
            finished.recv_timeout(DEADLINE).is_ok(),
            "{opened} of {THREADS} threads finished opening devices within {DEADLINE:?}"
        );
    }
}
