//! GPU abstraction layer for the Voltra engine.
//!
//! This crate knows about surfaces, devices and passes. It deliberately knows
//! *nothing* about windowing: callers hand it anything that converts into a
//! [`wgpu::SurfaceTarget`], which keeps `winit` out of the render layer.

pub mod camera;
pub mod context;
pub mod egui_backend;
pub mod mesh;
pub mod pass;
pub mod pipeline;
pub mod renderer;
pub mod shader;
pub mod target;
pub mod texture;

pub use camera::{Camera2D, CameraBinding};
pub use context::GpuContext;
pub use egui_backend::{EguiBackend, ScreenDescriptor};
pub use mesh::{Mesh, Vertex};
pub use renderer::Renderer;
pub use target::RenderTarget;
pub use texture::{Filter, Texture, TextureError};

// Re-exported so downstream crates never declare their own `wgpu` dependency
// and can't drift onto a different version.
pub use epaint;
pub use glam;
pub use wgpu;
