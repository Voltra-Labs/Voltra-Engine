//! A unit of work as the queue holds it.

/// A job with its borrows' lifetime erased: what the queue stores and a
/// worker runs. [`Scope::spawn`](crate::Scope::spawn) makes one by erasing
/// — see its safety argument — and a background task by owning everything
/// it touches.
pub(crate) struct Job(Box<dyn FnOnce() + Send + 'static>);

impl Job {
    /// A job that borrows nothing, which needs no argument to be queued.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn owned(f: Box<dyn FnOnce() + Send + 'static>) -> Self {
        Self(f)
    }

    /// # Safety
    ///
    /// `f` must have run, or been dropped, before anything it borrows for
    /// `'env` is freed. The queue gives no such guarantee; the caller must.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) unsafe fn erase<'env>(f: Box<dyn FnOnce() + Send + 'env>) -> Self {
        // SAFETY: the two types differ only in the trait object's lifetime
        // bound, which is not part of its layout; the caller guarantees the
        // borrows outlive every use.
        let f: Box<dyn FnOnce() + Send + 'static> = unsafe { std::mem::transmute(f) };
        Self(f)
    }

    pub(crate) fn run(self) {
        (self.0)();
    }
}
