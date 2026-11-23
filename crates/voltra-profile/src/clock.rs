//! The instant type: `std`'s natively, `web-time`'s on the web, where
//! `std::time::Instant` panics.

#[cfg(not(target_arch = "wasm32"))]
pub use std::time::Instant;
#[cfg(target_arch = "wasm32")]
pub use web_time::Instant;
