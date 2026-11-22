//! GPU abstraction layer for the Voltra engine.
//!
//! This crate knows about surfaces, devices and passes. It deliberately knows
//! *nothing* about windowing: callers hand it anything that converts into a
//! [`wgpu::SurfaceTarget`], which keeps `winit` out of the render layer.

pub mod bloom;
pub mod camera;
pub mod context;
pub mod egui_backend;
pub mod graph;
pub mod light_layers;
pub mod lights;
pub mod lines;
pub mod mesh;
pub mod normals;
pub mod particles;
pub mod pass;
pub mod pipeline;
pub mod post;
pub mod renderer;
pub mod scene_draw;
pub mod shader;
pub mod target;
pub mod texture;

pub use bloom::{BloomPass, BloomSettings};
pub use camera::{screen_projection, Camera2D, CameraBinding};
pub use context::GpuContext;
pub use egui_backend::{EguiBackend, ScreenDescriptor};
pub use light_layers::LayerPass;
pub use lights::{
    Cone, Cookie, GlobalLight, LightPass, Lighting, Occluder, PointLight, Reach, Shadow,
    ALL_LIGHT_LAYERS, DEFAULT_LIGHT_LAYERS,
};
pub use lines::{LineBatch, LineVertex, ViewportBinding};
pub use mesh::{Mesh, Vertex};
pub use normals::NormalPass;
pub use particles::{
    GpuParticles, ParticleCollider, ParticleLook, ParticlePipelines, ParticleResponse, SimStep,
};
pub use pass::MeshDraw;
pub use post::{PostPass, PostSettings};
pub use renderer::{Layer, Renderer};
pub use scene_draw::SceneDraw;
pub use target::RenderTarget;
pub use texture::{Filter, Texture, TextureError};

// Re-exported so downstream crates never declare their own `wgpu` dependency
// and can't drift onto a different version.
pub use epaint;
pub use glam;
pub use wgpu;
