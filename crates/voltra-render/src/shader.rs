//! WGSL shader modules.
//!
//! Engine shaders are embedded at compile time and live next to the code that
//! uses them, in `src/shaders/`. That is a different thing from `assets/`,
//! which is for game-authored content loaded at runtime — an engine that
//! cannot draw because a file is missing from disk is a worse engine.
//!
//! Hot reload arrives with the asset layer; until then, embedding keeps the
//! binary self-contained and removes any working-directory dependency.

/// Source of the built-in flat-colour triangle shader.
pub const FLAT_COLOR: &str = include_str!("shaders/flat_color.wgsl");

/// Source of the shader that draws egui's tessellated triangles.
pub const EGUI: &str = include_str!("shaders/egui.wgsl");

/// Source of the shader that widens line segments in screen space.
pub const LINES: &str = include_str!("shaders/lines.wgsl");

/// Compiles WGSL into a shader module.
///
/// `wgpu` validates and reports compilation errors through the device error
/// scope rather than a `Result`, so a broken shader surfaces as a logged
/// validation error, not a return value here.
pub fn create_module(device: &wgpu::Device, label: &str, source: &str) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    })
}
