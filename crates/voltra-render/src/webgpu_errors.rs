//! WebGPU's error scopes and uncaptured errors, read from the browser's
//! `GPUDevice` itself.
//!
//! wgpu 30's WebGPU backend turns a browser error into a `wgpu::Error`
//! with `dyn_ref::<GpuValidationError>()`, which fails for the
//! `GPUValidationError`s Chrome hands back, and then panics ("Unexpected
//! error", `backend/webgpu.rs`, `Error::from_js`) — from a popped error
//! scope and from `on_uncaptured_error` alike. So on WebGPU the engine
//! pushes and pops scopes and listens for uncaptured errors through the
//! `GPUDevice` (`wgpu::Device::as_webgpu`), reading only each error's
//! `message`. WebGL2 runs wgpu-core, whose errors convert, and takes
//! wgpu's own API.

use std::future::Future;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};

/// `object.name(args…)`, or the exception it threw.
fn call(object: &JsValue, name: &str, args: &[JsValue]) -> Result<JsValue, JsValue> {
    let function: js_sys::Function = js_sys::Reflect::get(object, &name.into())?.dyn_into()?;
    let list = js_sys::Array::new();
    for arg in args {
        list.push(arg);
    }
    function.apply(object, &list)
}

/// An error object's `message`, or what it says as a string.
fn message(error: &JsValue) -> String {
    js_sys::Reflect::get(error, &"message".into())
        .ok()
        .and_then(|m| m.as_string())
        .unwrap_or_else(|| format!("{error:?}"))
}

/// Pushes a validation scope on `device` (a `GPUDevice`).
pub(crate) fn push_validation_scope(device: &JsValue) {
    if let Err(e) = call(device, "pushErrorScope", &["validation".into()]) {
        log::error!("pushErrorScope failed: {}", message(&e));
    }
}

/// Pops `device`'s innermost scope: the message of what it caught, if
/// anything.
pub(crate) fn pop_scope(device: &JsValue) -> impl Future<Output = Option<String>> {
    let promise = call(device, "popErrorScope", &[])
        .ok()
        .and_then(|p| p.dyn_into::<js_sys::Promise>().ok());
    async move {
        let promise = promise?;
        match wasm_bindgen_futures::JsFuture::from(promise).await {
            Ok(error) if error.is_null() || error.is_undefined() => None,
            Ok(error) => Some(message(&error)),
            Err(e) => Some(message(&e)),
        }
    }
}

/// Hands each uncaptured error's message on `device` to `report`.
pub(crate) fn on_uncaptured(device: &JsValue, report: impl Fn(String) + 'static) {
    let closure = Closure::<dyn FnMut(JsValue)>::new(move |event: JsValue| {
        let error = js_sys::Reflect::get(&event, &"error".into()).unwrap_or(JsValue::NULL);
        report(message(&error));
    });
    if let Err(e) = js_sys::Reflect::set(device, &"onuncapturederror".into(), closure.as_ref()) {
        log::error!(
            "could not listen for uncaptured GPU errors: {}",
            message(&e)
        );
    }
    // The device holds it for the page's life, as wgpu's own handler does.
    closure.forget();
}
