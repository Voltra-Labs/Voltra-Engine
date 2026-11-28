//! What becomes of a GPU error no scope caught: logged, not a panic.
//!
//! wgpu's default handler panics on any uncaptured error
//! (`backend/wgpu_core.rs`, `default_error_handler`), so one invalid object
//! a game's content slipped past every check — a custom shader a driver
//! refuses — would take the whole game down. The engine's own draws are
//! pinned by tests; what reaches here is said, each distinct message once
//! (up to [`MAX_SAID`] of them), and counted. A lost device is not an error
//! here: it goes to the device's lost callback, which says so loudly
//! ([`GpuContext`](super::GpuContext)).
//!
//! **Headless and debug, an uncaptured error still panics** after it is
//! said: a headless device is a test's or a tool's, and an engine bug —
//! an invalid buffer, a pass out of step — must fail the test rather than
//! be counted. A game's own shaders never reach here: their banks make
//! them inside error scopes (`gpu_check`).

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// How many distinct messages are said; past it they are counted only, so
/// a stream of new ones (an id in each) cannot grow the set without end.
pub(crate) const MAX_SAID: usize = 64;

/// The uncaptured errors a device has had, shared with its handler.
#[derive(Debug, Default)]
pub(crate) struct UncapturedErrors {
    count: AtomicU64,
    /// The messages said, digits folded (`said_key`).
    said: Mutex<HashSet<String>>,
    /// Whether the cap was said to be reached.
    overflowed: AtomicBool,
    /// Whether one is a panic once said: a headless device in a debug
    /// build.
    strict: bool,
}

impl UncapturedErrors {
    /// Makes `device` log its uncaptured errors into a new tally — and,
    /// `strict`, panic after saying one — and answers the tally.
    pub(crate) fn install(device: &wgpu::Device, strict: bool) -> Arc<Self> {
        let errors = Arc::new(Self {
            strict,
            ..Self::default()
        });
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

    /// Counts one, logs `message` the first time it is seen (up to
    /// [`MAX_SAID`] messages), and panics if strict.
    fn say(&self, kind: &str, message: String) {
        self.count.fetch_add(1, Ordering::Relaxed);
        let key = said_key(&message);
        let (first, full) = self
            .said
            .lock()
            .map(|mut said| {
                if said.contains(&key) {
                    (false, false)
                } else if said.len() >= MAX_SAID {
                    (false, true)
                } else {
                    (said.insert(key), false)
                }
            })
            .unwrap_or((true, false));
        if first {
            log::error!("{kind}:\n{message}");
        } else if full && !self.overflowed.swap(true, Ordering::Relaxed) {
            log::error!("more than {MAX_SAID} distinct GPU errors: the rest are counted, not said");
        }
        if self.strict {
            panic!(
                "an uncaptured GPU error on a headless device (an engine bug): {kind}:\n{message}"
            );
        }
    }

    /// How many there have been.
    pub(crate) fn count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }
}

/// `message` with every run of digits folded to `#`: the same error on
/// another object (`Buffer (3, 1)`, `Buffer (4, 1)`) is one message.
fn said_key(message: &str) -> String {
    let mut key = String::with_capacity(message.len());
    let mut digits = false;
    for c in message.chars() {
        if c.is_ascii_digit() {
            if !digits {
                key.push('#');
            }
            digits = true;
        } else {
            key.push(c);
            digits = false;
        }
    }
    key
}

impl super::GpuContext {
    /// How many GPU errors no scope caught: each is logged (a distinct
    /// message once) instead of wgpu's default panic. 0 in a sound frame.
    pub fn uncaptured_errors(&self) -> u64 {
        self.errors.count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stray_uncaptured_error_panics_on_a_headless_device() {
        let Some(gpu) = crate::GpuContext::headless(4, 4) else {
            eprintln!("no adapter: skipping");
            return;
        };
        // An engine bug no scope expects: a mapped buffer of a size that is
        // no multiple of 4.
        let made = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            gpu.device().create_buffer(&wgpu::BufferDescriptor {
                label: Some("broken"),
                size: 3,
                usage: wgpu::BufferUsages::MAP_WRITE,
                mapped_at_creation: true,
            })
        }));
        assert!(made.is_err(), "a test must fail on it, not count it");
        assert_eq!(gpu.uncaptured_errors(), 1, "said and counted first");
    }

    #[test]
    fn distinct_messages_are_said_up_to_a_cap_and_counted_past_it() {
        let errors = UncapturedErrors::default();
        for i in 0..MAX_SAID * 3 {
            // The same error on another object folds into one message.
            errors.say("test", format!("Buffer ({i}, 1) is invalid"));
        }
        assert_eq!(errors.said.lock().map(|s| s.len()).unwrap_or(0), 1);
        for i in 0..MAX_SAID * 3 {
            let word: String = (0..4)
                .map(|k| char::from(b'a' + ((i >> (k * 4)) & 15) as u8))
                .collect();
            errors.say("test", format!("error {word}"));
        }
        let said = errors.said.lock().map(|s| s.len()).unwrap_or(0);
        assert_eq!(said, MAX_SAID, "bounded");
        assert_eq!(errors.count(), (MAX_SAID * 6) as u64, "every one counted");
    }
}
