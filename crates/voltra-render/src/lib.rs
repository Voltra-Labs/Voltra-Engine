//! GPU abstraction layer for the Voltra engine.
//!
//! This crate knows about surfaces, devices and passes. It deliberately knows
//! *nothing* about windowing: callers hand it anything that converts into a
//! [`wgpu::SurfaceTarget`], which keeps `winit` out of the render layer.

#![warn(missing_docs)]

pub mod bloom;
pub mod camera;
pub mod context;
pub mod egui_backend;
pub mod frame_buffers;
pub mod gpu_timer;
pub mod graph;
pub mod light_layers;
pub mod lights;
pub mod lines;
pub mod material;
pub mod mesh;
pub mod normals;
pub mod particles;
pub mod pass;
pub mod pipeline;
pub mod post;
pub mod quad;
pub mod readback;
pub mod render3d;
pub mod renderer;
pub mod scene_draw;
pub mod shader;
pub mod skinning;
pub mod target;
pub mod texture;

pub use bloom::{BloomPass, BloomSettings};
pub use camera::{screen_projection, CameraBinding, View2D};
pub use context::{GpuContext, TargetSupport};
pub use egui_backend::{EguiBackend, ScreenDescriptor};
pub use frame_buffers::FrameBuffers;
pub use gpu_timer::{GpuScope, GpuTimer};
pub use light_layers::LayerPass;
pub use lights::{
    Cone, Cookie, GlobalLight, LightPass, Lighting, Occluder, PointLight, Reach, Shadow,
    ALL_LIGHT_LAYERS, DEFAULT_LIGHT_LAYERS,
};
pub use lines::{LineBatch, LineVertex, ViewportBinding};
pub use mesh::{Mesh2D, MeshBuffers, Primitives, Vertex2D};
pub use normals::NormalPass;
pub use particles::{
    FrameTiming, GpuParticles, ParticleCollider, ParticleLook, ParticlePipelines, ParticleResponse,
    ParticleTables, SimStep, SpawnShape,
};
pub use pass::{MaterialDraw, MeshDraw};
pub use pipeline::SpritePipeline;
pub use post::{Grade, PictureAlpha, PostPass, PostSettings};
pub use quad::QuadInstance;
pub use readback::read_rgba8;
pub use renderer::{Layer, Placed, Renderer};
pub use scene_draw::SceneDraw;
pub use skinning::{SkinPipeline, SkinVertex, SkinnedMesh};
pub use target::RenderTarget;
pub use texture::{Filter, Texture, TextureError};

// The names 0.4 used, for out-of-tree code moving to 0.5 (`mesh.rs`).
// `Camera2D` has none: it is `voltra_scene::Camera2D` now, and an alias
// here would make a glob import of both crates ambiguous.
#[allow(deprecated)]
pub use mesh::{Mesh, Vertex};

// Re-exported so downstream crates never declare their own `wgpu` dependency
// and can't drift onto a different version.
pub use epaint;
pub use glam;
pub use wgpu;
