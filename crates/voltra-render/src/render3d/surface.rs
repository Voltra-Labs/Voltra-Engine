//! Surface materials: glTF 2.0's metallic-roughness inputs, lit by
//! Filament's BRDF, or a material's own WGSL `surface()` starting from
//! them — the 3D twin of the sprite material ([`material`](crate::material)),
//! drawn by [`Pass3D`](super::Pass3D).
//!
//! The numbers ([`StandardSurface`]), the images ([`SurfaceTextures`]) and,
//! for a custom surface, its [`CustomShading`] become a
//! [`SurfaceMaterial3D`]; [`compose_surface`] writes a custom shader and
//! [`SurfaceShader`] checks it; a [`SurfaceBank`] holds the pipeline
//! variants by [`SurfaceKey`]; `brdf` is the shader's lighting on the CPU.

mod bank;
pub mod brdf;
mod compose;
mod custom;
mod layouts;
mod material;
mod shader;
mod standard;
mod uniform;

pub use bank::{AlphaClass, SurfaceBank, SurfaceKey, SurfacePass, SurfaceVertex, KEEP_USES};
pub use compose::{
    compose_surface, standard_source, SurfaceLighting, CUSTOM_TEXTURES, SURFACE_ENTRIES,
    SURFACE_ENTRY,
};
pub use custom::{CustomImage, CustomShading, TextureSpace};
pub use layouts::{SurfaceLayouts, CUSTOM_BLOCK_BINDING, CUSTOM_TEXTURE_BINDING, SURFACE_TEXTURES};
pub use material::{SurfaceMaterial3D, SurfaceTextures, SURFACE_VIEWS};
pub use shader::SurfaceShader;
pub use standard::{AlphaMode, StandardSurface, UvTransform};
pub use uniform::SurfaceUniform;

#[cfg(test)]
mod tests {
    use wgpu::naga;

    use super::*;

    /// What WebGL2 guarantees a fragment stage, and the share the plan
    /// leaves the standard surface (ARCHITECTURE "Materials": five
    /// standard, four custom, the shadow array, the specular cube and the
    /// BRDF table — 12 of 16).
    const WEBGL2_FRAGMENT_TEXTURES: u32 = 16;
    const PLANNED_TEXTURES: u32 = 12;

    fn module() -> naga::Module {
        let source = &standard_source();
        let module = naga::front::wgsl::parse_str(source)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(source)));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("{}", e.emit_to_string(source)));
        module
    }

    #[test]
    fn the_surface_shader_validates_within_the_webgl2_budgets() {
        let module = module();
        let limits = wgpu::Limits::downlevel_webgl2_defaults();
        assert_eq!(
            limits.max_sampled_textures_per_shader_stage,
            WEBGL2_FRAGMENT_TEXTURES
        );
        let textures = module
            .global_variables
            .iter()
            .filter(|(_, g)| matches!(module.types[g.ty].inner, naga::TypeInner::Image { .. }))
            .count() as u32;
        // Every slot the layout binds — five standard, four custom — and
        // room for 140's shadow array and 141's cube and table.
        let bound = SURFACE_TEXTURES + CUSTOM_TEXTURES as u32;
        assert_eq!(textures, SURFACE_TEXTURES, "the standard samples its five");
        assert_eq!(bound, 9);
        assert!(bound + 3 <= PLANNED_TEXTURES, "room for 140 and 141");
        let samplers = module
            .global_variables
            .iter()
            .filter(|(_, g)| matches!(module.types[g.ty].inner, naga::TypeInner::Sampler { .. }))
            .count() as u32;
        assert!(samplers <= limits.max_samplers_per_shader_stage);
        // The blocks, far under a 16 KiB uniform block.
        for size in [
            size_of::<SurfaceUniform>(),
            size_of::<crate::render3d::FrameUniform3D>(),
        ] {
            assert!(
                size as u64 <= limits.max_uniform_buffer_binding_size,
                "{size}"
            );
            assert_eq!(size % 16, 0);
        }
        // What the vertex stage hands the fragment stage: five locations
        // of WebGL2's 15.
        let vertex = module
            .entry_points
            .iter()
            .find(|e| e.name == "vs_main")
            .expect("a vertex stage");
        let out = vertex.function.result.as_ref().expect("an output");
        let naga::TypeInner::Struct { members, .. } = &module.types[out.ty].inner else {
            panic!("a struct");
        };
        let locations = members
            .iter()
            .filter(|m| matches!(m.binding, Some(naga::Binding::Location { .. })))
            .count() as u32;
        assert_eq!(locations, 5);
        assert!(locations <= limits.max_inter_stage_shader_variables);
        for entry in SURFACE_ENTRIES {
            assert!(
                module.entry_points.iter().any(|e| e.name == entry),
                "{entry}"
            );
        }
    }
}

#[cfg(test)]
mod custom_tests;
