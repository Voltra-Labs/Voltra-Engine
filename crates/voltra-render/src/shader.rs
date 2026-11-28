//! WGSL shader modules.
//!
//! Engine shaders are embedded at compile time and live next to the code that
//! uses them, in `src/shaders/`. That is a different thing from `assets/`,
//! which is for game-authored content loaded at runtime — an engine that
//! cannot draw because a file is missing from disk is a worse engine.
//!
//! Hot reload arrives with the asset layer; until then, embedding keeps the
//! binary self-contained and removes any working-directory dependency.

/// The sprite batch's quads as instances: `QuadInput` and `quad_corner`,
/// put before every shader that draws them. See
/// [`QuadInstance`](crate::quad::QuadInstance).
pub const QUAD: &str = include_str!("shaders/quad.wgsl");

/// Source of the built-in flat-colour shader: the sprite batch's triangles
/// and quads.
pub const FLAT_COLOR: &str = concat!(
    include_str!("shaders/quad.wgsl"),
    "\n",
    include_str!("shaders/flat_color.wgsl")
);

/// Standard surfaces, instanced and lit: glTF's metallic-roughness inputs
/// through Filament's BRDF, under the frame's lights, fogged. See
/// [`SurfaceBank`](crate::render3d::SurfaceBank).
pub const SURFACE_3D: &str = concat!(
    include_str!("shaders/frame3d.wgsl"),
    "\n",
    include_str!("shaders/surface3d.wgsl"),
    "\n",
    include_str!("shaders/surface3d_light.wgsl"),
    "\n",
    include_str!("shaders/environment3d.wgsl")
);

/// The 3D sky: a gradient by elevation drawn behind everything at the far
/// plane. See `render3d::sky`.
pub const SKY_3D: &str = concat!(
    include_str!("shaders/frame3d.wgsl"),
    "\n",
    include_str!("shaders/environment3d.wgsl"),
    "\n",
    include_str!("shaders/sky3d.wgsl")
);

/// Source of the shader that draws egui's tessellated triangles.
pub const EGUI: &str = include_str!("shaders/egui.wgsl");

/// Source of the shader that widens line segments in screen space.
pub const LINES: &str = include_str!("shaders/lines.wgsl");

/// The full-screen post-processing pass of a 2D picture: grading and
/// vignette, and the overlay's unpremultiplied grade.
pub const POST: &str = concat!(
    include_str!("shaders/grade.wgsl"),
    "\n",
    include_str!("shaders/post.wgsl")
);

/// The full-screen post-processing pass of a 3D picture: exposure, the
/// tone curve, then the same grade. See [`Grade::Tonemap`](crate::post::Grade).
pub const TONEMAP: &str = concat!(
    include_str!("shaders/grade.wgsl"),
    "\n",
    include_str!("shaders/tonemap.wgsl")
);

/// Bloom's prefilter, downsample and upsample. See [`BloomPass`](crate::bloom::BloomPass).
pub const BLOOM: &str = include_str!("shaders/bloom.wgsl");

/// Adds 2D point lights onto the light map. See [`LightPass`](crate::lights::LightPass).
pub const LIGHTS: &str = include_str!("shaders/lights.wgsl");

/// Adds global 2D lights onto the light map. See [`LightPass`](crate::lights::LightPass).
pub const LIGHTS_GLOBAL: &str = include_str!("shaders/lights_global.wgsl");

/// Multiplies the scene by the light map. See [`LightPass`](crate::lights::LightPass).
pub const LIGHTS_COMPOSITE: &str = include_str!("shaders/lights_composite.wgsl");

/// Draws the sprite batch's light layers. See [`LayerPass`](crate::light_layers::LayerPass).
pub const SPRITE_LAYERS: &str = concat!(
    include_str!("shaders/quad.wgsl"),
    "\n",
    include_str!("shaders/sprite_layers.wgsl")
);

/// Draws the sprite batch's normals. See [`NormalPass`](crate::normals::NormalPass).
pub const SPRITE_NORMALS: &str = concat!(
    include_str!("shaders/quad.wgsl"),
    "\n",
    include_str!("shaders/sprite_normals.wgsl")
);

/// Draws occluding edges into the 2D shadow map. See [`LightPass`](crate::lights::LightPass).
pub const LIGHT_SHADOWS: &str = include_str!("shaders/light_shadows.wgsl");

/// Steps GPU particles. See [`ParticlePipelines`](crate::particles::ParticlePipelines).
pub const PARTICLES_SIM: &str = include_str!("shaders/particles_sim.wgsl");

/// Draws GPU particles. See [`ParticlePipelines`](crate::particles::ParticlePipelines).
pub const PARTICLES_DRAW: &str = include_str!("shaders/particles_draw.wgsl");

/// A GPU emitter's light layers: a fragment stage appended to
/// [`PARTICLES_DRAW`]. See [`LayerPass`](crate::light_layers::LayerPass).
pub const PARTICLE_LAYERS: &str = include_str!("shaders/particle_layers.wgsl");

/// Draws sprite meshes deformed by bones. See
/// [`SkinPipeline`](crate::skinning::SkinPipeline).
pub const SKINNED: &str = include_str!("shaders/skinned.wgsl");

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
