//! `Send` and `Sync` where jobs cross threads, and nothing where they do
//! not.
//!
//! On the web (`wasm32` without atomics) there are no threads: a job runs
//! where it is spawned, and the GPU and audio handles a job may borrow are
//! not `Send` there. Bounds written with these traits hold natively exactly
//! as `Send`/`Sync` do, and ask nothing on the web — wgpu's
//! `WasmNotSendSync`, for the same reason.

/// `Send` natively; anything on the web.
#[cfg(not(target_arch = "wasm32"))]
pub trait MaybeSend: Send {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + ?Sized> MaybeSend for T {}

/// `Send` natively; anything on the web.
#[cfg(target_arch = "wasm32")]
pub trait MaybeSend {}
#[cfg(target_arch = "wasm32")]
impl<T: ?Sized> MaybeSend for T {}

/// `Sync` natively; anything on the web.
#[cfg(not(target_arch = "wasm32"))]
pub trait MaybeSync: Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Sync + ?Sized> MaybeSync for T {}

/// `Sync` natively; anything on the web.
#[cfg(target_arch = "wasm32")]
pub trait MaybeSync {}
#[cfg(target_arch = "wasm32")]
impl<T: ?Sized> MaybeSync for T {}
