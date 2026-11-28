//! A GPU object's validation, caught in an error scope rather than by the
//! device's handler, and read without blocking.
//!
//! naga's native check is not the browser's: naga 30 lifts WGSL's fragment
//! uniformity rule (`DISABLE_UNIFORMITY_REQ_FOR_FRAGMENT_STAGE`), Tint does
//! not, and a module Tint refuses makes every pipeline built from it — and
//! every command buffer drawing with one — invalid: the whole WebGPU frame
//! goes black. So a custom surface's module and pipelines are made inside
//! a validation scope, and drawn only once the scope has answered that
//! nothing went wrong. On wgpu-core the answer is ready at once; on WebGPU
//! it is a promise, polled each shot.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

/// What a popped scope answers: the message of what it caught, if anything.
#[cfg(not(target_arch = "wasm32"))]
type ScopeFuture = Pin<Box<dyn Future<Output = Option<String>> + Send>>;
#[cfg(target_arch = "wasm32")]
type ScopeFuture = Pin<Box<dyn Future<Output = Option<String>>>>;

/// Where one object's validation stands.
pub(super) enum GpuCheck {
    /// The scope has not answered yet: not to be drawn with.
    Pending(ScopeFuture),
    /// Valid: drawn with.
    Passed,
    /// Refused, with why.
    Failed(String),
}

impl std::fmt::Debug for GpuCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending(_) => f.write_str("Pending"),
            Self::Passed => f.write_str("Passed"),
            Self::Failed(why) => f.debug_tuple("Failed").field(why).finish(),
        }
    }
}

impl GpuCheck {
    /// What `make` makes on `device`, checked in a validation scope: pending
    /// until [`poll`](Self::poll)ed (wgpu-core answers at the first).
    ///
    /// On WebGPU the scope is the browser's own (`webgpu_errors`): wgpu 30
    /// panics converting the error a WebGPU scope hands back.
    pub(super) fn scoped<T>(device: &wgpu::Device, make: impl FnOnce() -> T) -> (T, Self) {
        #[cfg(target_arch = "wasm32")]
        if let Some(js) = device.as_webgpu() {
            crate::webgpu_errors::push_validation_scope(js.as_ref());
            let made = make();
            let popped = crate::webgpu_errors::pop_scope(js.as_ref());
            return (made, Self::Pending(Box::pin(popped)));
        }
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let made = make();
        let popped = scope.pop();
        (
            made,
            Self::Pending(Box::pin(async move { popped.await.map(|e| e.to_string()) })),
        )
    }

    /// Reads the scope's answer if it has come, without waiting. Answers
    /// the failure the first time it is seen.
    pub(super) fn poll(&mut self) -> Option<&str> {
        let Self::Pending(future) = self else {
            return None;
        };
        let mut context = Context::from_waker(Waker::noop());
        match future.as_mut().poll(&mut context) {
            Poll::Pending => None,
            Poll::Ready(None) => {
                *self = Self::Passed;
                None
            }
            Poll::Ready(Some(error)) => {
                *self = Self::Failed(error);
                match self {
                    Self::Failed(why) => Some(why),
                    _ => None,
                }
            }
        }
    }

    /// Whether it may be drawn with.
    pub(super) fn passed(&self) -> bool {
        matches!(self, Self::Passed)
    }
}
