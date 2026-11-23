//! Buffers made for one frame, destroyed once that frame is submitted.
//!
//! On the web dropping a `wgpu::Buffer` frees nothing: the GPU memory goes
//! when JavaScript's collector gets to the wrapper, and a tiny wrapper gives
//! it no reason to hurry. A pass that uploads its instances afresh every
//! frame then exhausts the browser's mapping memory within seconds (Chrome:
//! "createBuffer failed … too large for the implementation"). Destroying
//! explicitly is what WebGPU asks for, and it is safe after the submit:
//! `destroy` waits for the work already queued. Natively it frees the same
//! memory a frame sooner than the drop would.
//!
//! The record functions that make these buffers finish before the submit,
//! so they cannot destroy their own; this list holds each until
//! [`GpuContext::submit`](crate::GpuContext::submit) has gone by.

use std::sync::{Mutex, PoisonError};

use wgpu::util::DeviceExt;

/// The frame's own buffers, kept until the frame is submitted.
#[derive(Default)]
pub struct FrameBuffers {
    live: Mutex<Vec<wgpu::Buffer>>,
}

impl FrameBuffers {
    /// A buffer holding `desc`'s contents until the next submit.
    pub fn create(
        &self,
        device: &wgpu::Device,
        desc: &wgpu::util::BufferInitDescriptor<'_>,
    ) -> wgpu::Buffer {
        let buffer = device.create_buffer_init(desc);
        self.live
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(buffer.clone());
        buffer
    }

    /// Destroys every buffer made since the last call. Only after the
    /// commands that use them are submitted.
    pub(crate) fn retire(&self) {
        let retired =
            std::mem::take(&mut *self.live.lock().unwrap_or_else(PoisonError::into_inner));
        for buffer in retired {
            buffer.destroy();
        }
    }
}
