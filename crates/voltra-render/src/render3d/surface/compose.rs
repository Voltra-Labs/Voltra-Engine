//! A surface material's whole shader: the engine's prelude — bindings,
//! vertex stage, `SurfaceInput`, `Surface`, `standard_surface`, the
//! lighting and the fog — the material's uniforms and textures, its own
//! `surface()`, and each pass's fragment stage around it.
//!
//! The 3D twin of [`material::compose`](crate::material::compose): a sprite
//! material writes `fn fragment`, a surface material `fn surface(in:
//! SurfaceInput) -> Surface`, and the engine keeps the vertex stage, the
//! lighting and every pass. The standard surface is the same composition
//! of a `surface()` that returns `standard_surface(in)`.

use std::collections::HashSet;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use super::layouts::{CUSTOM_BLOCK_BINDING, CUSTOM_TEXTURE_BINDING};
use crate::material::Layout;

/// The function a surface material's WGSL defines.
pub const SURFACE_ENTRY: &str = "surface";

/// How many named textures a custom surface may sample, beside the
/// standard five: WebGL2 gives a fragment stage 16, and the plan keeps 12
/// (five standard, four custom, the shadow array, the specular cube and
/// the BRDF table — ARCHITECTURE "Materials").
pub const CUSTOM_TEXTURES: usize = 4;

/// Whether the engine lights what a material's `surface()` returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum SurfaceLighting {
    /// Lit by the frame's light and ambient (the standard block's `unlit`
    /// still turns it off).
    #[default]
    Lit,
    /// Drawn as its base colour, the lighting compiled out: Godot's
    /// `unshaded`.
    Unlit,
}

/// The `surface()` of a material with no shader: its `standard` block.
const STANDARD: &str =
    "fn surface(in: SurfaceInput) -> Surface {\n    return standard_surface(in);\n}\n";

/// The fragment entry points, after the material's source. `surface_color`
/// is the lighting (or not) the composer chose.
const ENTRIES: &str = r#"
// The engine's: each pass's fragment stage around `surface()`.

// Opaque: alpha 1 whatever the surface's, so a 3D overlay is see-through
// only where nothing was drawn.
@fragment
fn fs_opaque(vertex: VertexOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let in = surface_input(vertex, front);
    let s = surface(in);
    return vec4<f32>(surface_color(s, in), 1.0);
}

// Cut out under the cutoff, opaque elsewhere: one sample a pixel.
@fragment
fn fs_mask(vertex: VertexOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let in = surface_input(vertex, front);
    let s = surface(in);
    let color = surface_color(s, in);
    if s.alpha < surface_cutoff() {
        discard;
    }
    return vec4<f32>(color, 1.0);
}

// Cut out by alpha to coverage under MSAA: alpha sharpened to a pixel-wide
// ramp about the cutoff (Ben Golus's), which the samples cover in steps.
// The colour is written whole: the samples covered already weigh it by the
// coverage when they resolve, so premultiplying it too would darken every
// edge to coverage². The alpha output is the coverage (alpha to coverage
// reads it); the pipeline keeps the larger of it and what is there (`Max`),
// so over an opaque clear a covered sample stays opaque.
@fragment
fn fs_mask_coverage(vertex: VertexOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let in = surface_input(vertex, front);
    let s = surface(in);
    let ramp = max(fwidth(s.alpha), 1.0e-4);
    let coverage = clamp((s.alpha - surface_cutoff()) / ramp + 0.5, 0.0, 1.0);
    return vec4<f32>(surface_color(s, in), coverage);
}

// Blended: premultiplied, as the 3D target and the composite are.
@fragment
fn fs_blend(vertex: VertexOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let in = surface_input(vertex, front);
    let s = surface(in);
    let alpha = clamp(s.alpha, 0.0, 1.0);
    return vec4<f32>(surface_color(s, in) * alpha, alpha);
}

// No colour: `surface()` run for its alpha alone, discarded under the
// cutoff. A masked surface's fragment stage in a pass that writes depth
// only, so a cut-out casts (stage 140's shadows) and occludes as it is
// drawn; an opaque one needs no fragment stage there.
@fragment
fn fs_cutout(vertex: VertexOut, @builtin(front_facing) front: bool) {
    let in = surface_input(vertex, front);
    let s = surface(in);
    if s.alpha < surface_cutoff() {
        discard;
    }
}
"#;

/// The fragment entry points every composed surface shader has, by pass:
/// the main pass's four and the depth-only cut-out.
pub const SURFACE_ENTRIES: [&str; 5] = [
    "fs_opaque",
    "fs_mask",
    "fs_mask_coverage",
    "fs_blend",
    "fs_cutout",
];

/// The whole WGSL for a surface material whose own source is `source`,
/// whose uniforms are laid out as `layout`, sampling the named `textures`
/// (each a WGSL name the source reads, at most [`CUSTOM_TEXTURES`]), lit as
/// `lighting` says.
///
/// The source sees everything in the prelude — `frame`, `standard`,
/// `surface_sampler`, `SurfaceInput`, `Surface`, `standard_surface(in)`,
/// `surface_uv(in)`, `light_surface(s, in)`, `apply_fog(color, world)`,
/// `sky_color(direction)` — plus `material` (its
/// uniforms) and its textures, and must define `fn surface(in:
/// SurfaceInput) -> Surface`. Its text comes first, so naga reports an
/// error in it at the material file's own line numbers. `Err` when there
/// are too many textures or a name is no WGSL identifier; whether the result compiles is
/// [`SurfaceShader::new`](super::SurfaceShader::new)'s to say.
pub fn compose_surface(
    source: &str,
    layout: &Layout,
    textures: &[&str],
    lighting: SurfaceLighting,
) -> Result<String, String> {
    if textures.len() > CUSTOM_TEXTURES {
        return Err(format!(
            "{} textures; a surface material samples at most {CUSTOM_TEXTURES} of its own",
            textures.len()
        ));
    }
    let mut declared = String::new();
    for (i, name) in textures.iter().enumerate() {
        if !crate::material::is_identifier(name) {
            return Err(format!("texture {name:?} is not a name WGSL takes"));
        }
        if engine_names().contains(*name) {
            return Err(format!(
                "texture {name:?} is a name the engine's prelude declares; call it something \
                 else"
            ));
        }
        declared.push_str(&format!(
            "@group(1) @binding({}) var {name}: texture_2d<f32>;\n",
            CUSTOM_TEXTURE_BINDING + i as u32
        ));
    }
    let color = match lighting {
        SurfaceLighting::Lit => "light_surface(s, in)",
        SurfaceLighting::Unlit => "unlit_surface(s)",
    };
    // The material's own first: WGSL resolves module-scope names in any
    // order, and so an error in it is reported at its own line numbers.
    Ok(format!(
        "{source}\n\n// The engine's: the material's uniforms and textures.\n{block}@group(1) @binding({CUSTOM_BLOCK_BINDING})\nvar<uniform> material: Material;\n{declared}\n{prelude}\n// What `surface()` returns becomes: lit, or as it is, then fogged — the\n// cap after the fog, so a fog colour past it cannot overflow either.\nfn surface_color(s: Surface, in: SurfaceInput) -> vec3<f32> {{\n    return min(apply_fog({color}, in.world), vec3<f32>(MAX_LIGHT));\n}}\n{ENTRIES}",
        prelude = crate::shader::SURFACE_3D,
        block = layout.wgsl_struct_aligned(),
    ))
}

/// Every module-scope name the composer declares around a material's
/// source — the prelude's functions, types, constants and bindings, the
/// uniform block and the entry points — but `surface`, which the source
/// defines. A texture named one of them would be redefined in a line the
/// author never wrote; a clash with the source's own names is naga's to
/// report, at the source's line.
fn engine_names() -> &'static HashSet<String> {
    static NAMES: LazyLock<HashSet<String>> = LazyLock::new(|| {
        let module = wgpu::naga::front::wgsl::parse_str(crate::shader::SURFACE_3D)
            .unwrap_or_else(|e| panic!("the engine's prelude parses: {e}"));
        let mut names: HashSet<String> = module
            .functions
            .iter()
            .filter_map(|(_, f)| f.name.clone())
            .chain(
                module
                    .global_variables
                    .iter()
                    .filter_map(|(_, g)| g.name.clone()),
            )
            .chain(module.constants.iter().filter_map(|(_, c)| c.name.clone()))
            .chain(module.types.iter().filter_map(|(_, t)| t.name.clone()))
            .chain(module.entry_points.iter().map(|e| e.name.clone()))
            .collect();
        names.extend(SURFACE_ENTRIES.iter().map(|e| (*e).to_owned()));
        names.extend(["material", "Material", "surface_color"].map(str::to_owned));
        names
    });
    &NAMES
}

/// The standard surface's whole shader: [`compose_surface`] of a
/// `surface()` that returns `standard_surface(in)`, no uniforms, no
/// textures, lit.
pub fn standard_source() -> String {
    let empty = Layout::new(&Default::default()).expect("no uniforms fit");
    compose_surface(STANDARD, &empty, &[], SurfaceLighting::Lit)
        .expect("no textures is within the budget")
}
