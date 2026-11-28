//! What becomes of a GPU error no scope caught: logged, not a panic.
//!
//! wgpu's default handler panics on any uncaptured error
//! (`backend/wgpu_core.rs`, `default_error_handler`), so one invalid object
//! a game's content slipped past every check — a custom shader a driver
//! refuses — would take the whole game down. The engine's own draws are
//! pinned by tests; what reaches here is said, each distinct message once,
//! and counted. A lost device is not an error here: it goes to the device's
//! lost callback, which says so loudly ([`GpuContext`](super::GpuContext)).

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// The uncaptured errors a device has had, shared with its handler.
#[derive(Debug, Default)]
pub(crate) struct UncapturedErrors {
    count: AtomicU64,
    said: Mutex<HashSet<String>>,
}

impl UncapturedErrors {
    /// Makes `device` log its uncaptured errors into a new tally instead of
    /// panicking, and answers the tally.
    pub(crate) fn install(device: &wgpu::Device) -> Arc<Self> {
        let errors = Arc::new(Self::default());
        let tally = Arc::clone(&errors);
        // On WebGPU through the browser's device: wgpu 30 panics converting
        // its errors (`crate::webgpu_errors`).
        #[cfg(target_arch = "wasm32")]
        if let Some(js) = device.as_webgpu() {
            crate::webgpu_errors::on_uncaptured(js.as_ref(), move |message| {
                tally.say("a GPU error; what it touched may not draw", message);
            });
            return errors;
        }
        device.on_uncaptured_error(Arc::new(move |error: wgpu::Error| tally.report(&error)));
        errors
    }

    fn report(&self, error: &wgpu::Error) {
        let (kind, message) = match error {
            wgpu::Error::OutOfMemory { .. } => ("out of GPU memory", error.to_string()),
            wgpu::Error::Validation { description, .. } => (
                "a GPU validation error; what it touched may not draw",
                description.clone(),
            ),
            wgpu::Error::Internal { description, .. } => {
                ("an internal GPU error", description.clone())
            }
        };
        self.say(kind, message);
    }

    /// Counts one, and logs `message` the first time it is seen.
    fn say(&self, kind: &str, message: String) {
        self.count.fetch_add(1, Ordering::Relaxed);
        let first = self
            .said
            .lock()
            .map(|mut said| said.insert(message.clone()))
            .unwrap_or(true);
        if first {
            log::error!("{kind}:\n{message}");
        }
    }

    /// How many there have been.
    pub(crate) fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }
}

impl super::GpuContext {
    /// How many GPU errors no scope caught: each is logged (a distinct
    /// message once) instead of wgpu's default panic. 0 in a sound frame.
    pub fn uncaptured_errors(&self) -> u64 {
        self.errors.count()
    }
}
