//! One wgpu device opened at a time in a process.
//!
//! With every backend enabled, `wgpu::Instance::new` initialises the Vulkan
//! and the OpenGL drivers. On Windows with NVIDIA's drivers, two threads doing
//! that at once can deadlock inside the drivers: a test binary whose parallel
//! tests each open a headless device hung about one run in five (stage 119's
//! hardware pass). Opening is rare and short, so it is serialised: whoever
//! creates an instance, an adapter and a device holds [`opening`] across all
//! three.

use std::sync::{Mutex, MutexGuard, PoisonError};

static OPENING: Mutex<()> = Mutex::new(());

/// The process-wide lock to hold while creating a `wgpu::Instance` and
/// requesting its adapter and device. Every device `voltra-render` opens takes
/// it; code opening a device of its own (a test kit) takes it too.
///
/// A panic while it was held leaves nothing to repair, so a poisoned lock is
/// taken as it is.
pub fn opening() -> MutexGuard<'static, ()> {
    OPENING.lock().unwrap_or_else(PoisonError::into_inner)
}
