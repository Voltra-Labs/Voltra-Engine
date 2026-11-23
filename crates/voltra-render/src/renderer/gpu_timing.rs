//! The renderer's GPU timings: switched on and off, and read back.

use super::Renderer;

impl Renderer {
    /// Times each pass on the GPU from the next frame on, or stops — only
    /// where the adapter can ([`gpu_timing_supported`]).
    ///
    /// [`gpu_timing_supported`]: Self::gpu_timing_supported
    pub fn set_gpu_timing(&mut self, on: bool) {
        self.gpu_timer.set_enabled(on);
    }

    /// Whether this adapter can time passes.
    pub fn gpu_timing_supported(&self) -> bool {
        self.gpu_timer.is_supported()
    }

    /// The pass timings that have come back from the GPU since the last
    /// call, in milliseconds, by pass name. Frames late, and never waited
    /// for.
    pub fn gpu_timings(&mut self) -> Vec<(&'static str, f64)> {
        self.gpu_timer.collect(self.ctx.device())
    }
}
