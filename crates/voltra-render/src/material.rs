//! Sprite materials: a WGSL fragment function with declared uniforms,
//! compiled into a variant of the sprite pipeline.
//!
//! Godot's `CanvasItemMaterial` with a shader, Unity's sprite material: the
//! vertex stage, the camera and the texture binding stay the engine's; the
//! author writes only `fn fragment(in: Fragment) -> vec4<f32>`, reading the
//! sprite's texture (`albedo`, `albedo_sampler`), the vertex colour and UV
//! from `in`, and its own uniforms from `material`. See [`compose`].

mod bank;
mod compose;
mod pipeline;
mod uniforms;

pub use bank::{MaterialBank, MaterialRequest};
pub use compose::{compose, validate, FRAGMENT_ENTRY};
pub use pipeline::{create_material_pipeline, uniform_layout};
pub(crate) use uniforms::is_identifier;
pub use uniforms::{Layout, MaterialUniforms, Uniform, BLOCK_SIZE};
