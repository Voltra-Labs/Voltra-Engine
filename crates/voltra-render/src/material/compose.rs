//! A material's whole shader: the engine's sprite vertex stage and
//! bindings, the material's uniform struct, and its fragment function.

use super::uniforms::Layout;

/// The function a material's WGSL defines.
pub const FRAGMENT_ENTRY: &str = "fragment";

/// What comes before a material's own source: the bindings of the sprite
/// pipeline — group 0 the camera, group 1 the sprite's texture — the
/// vertex stage, and `Fragment`, what the fragment function is handed.
const PRELUDE: &str = r#"// The engine's: the sprite pipeline's bindings and vertex stage.
struct Camera {
    view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var albedo: texture_2d<f32>;

@group(1) @binding(1)
var albedo_sampler: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,
}

// What `fragment` is handed: the pixel's place in the target, the sprite's
// vertex colour (its tint) and the texture coordinate.
struct Fragment {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> Fragment {
    var out: Fragment;
    out.position = camera.view_proj * vec4<f32>(in.position, 0.0, 1.0);
    out.color = in.color;
    out.uv = in.uv;
    return out;
}

// What the sprite pipeline would have drawn: the texel times the tint.
fn sprite_color(in: Fragment) -> vec4<f32> {
    return textureSample(albedo, albedo_sampler, in.uv) * in.color;
}
"#;

/// The whole WGSL for a material whose fragment source is `fragment` and
/// whose uniforms are laid out as `layout`: the prelude, `struct Material`
/// bound at group 2, the material's source, and the entry point calling its
/// `fragment`.
///
/// The material's source sees `albedo`, `albedo_sampler`, `camera`,
/// `material` (its uniforms), `Fragment` and `sprite_color(in)`, and must
/// define `fn fragment(in: Fragment) -> vec4<f32>`.
pub fn compose(fragment: &str, layout: &Layout) -> String {
    format!(
        "{PRELUDE}\n// The material's uniforms.\n{}\n@group(2) @binding(0)\nvar<uniform> material: Material;\n\n// The material's own.\n{fragment}\n\n@fragment\nfn fs_main(in: Fragment) -> @location(0) vec4<f32> {{\n    return {FRAGMENT_ENTRY}(in);\n}}\n",
        layout.wgsl_struct()
    )
}

/// Whether `source` — a [`compose`]d shader — parses and validates, with
/// what is wrong and where when it does not. Checked before the pipeline is
/// made, because `wgpu` reports a broken shader to an error scope rather
/// than to its caller, and a broken material must fall back, not take the
/// device down.
pub fn validate(source: &str) -> Result<(), String> {
    use wgpu::naga;
    let module = naga::front::wgsl::parse_str(source).map_err(|e| e.emit_to_string(source))?;
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .map_err(|e| e.emit_to_string(source))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::material::Uniform;

    fn layout() -> Layout {
        let uniforms: BTreeMap<String, Uniform> = [
            ("flash".to_owned(), Uniform::Float(0.0)),
            (
                "flash_color".to_owned(),
                Uniform::Color((1.0, 1.0, 1.0, 1.0)),
            ),
        ]
        .into_iter()
        .collect();
        Layout::new(&uniforms).expect("fits")
    }

    #[test]
    fn a_material_composes_into_a_valid_shader() {
        let source = compose(
            "fn fragment(in: Fragment) -> vec4<f32> {\n    let base = sprite_color(in);\n    return vec4<f32>(mix(base.rgb, material.flash_color.rgb, material.flash), base.a);\n}",
            &layout(),
        );
        validate(&source).expect("valid");
    }

    #[test]
    fn a_broken_material_says_what_and_where() {
        let source = compose(
            "fn fragment(in: Fragment) -> vec4<f32> { return material.nope; }",
            &layout(),
        );
        let error = validate(&source).expect_err("no such field");
        assert!(error.contains("nope"), "{error}");
        let missing = compose("fn other() {}", &layout());
        assert!(validate(&missing).is_err(), "no fragment function");
    }
}
