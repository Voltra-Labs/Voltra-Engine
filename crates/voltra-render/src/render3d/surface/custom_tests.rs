//! Composing and checking a custom surface shader.

use std::collections::BTreeMap;
use std::sync::Arc;

use wgpu::naga;

use super::*;
use crate::material::{Layout, Uniform};

const TINTED: &str = "fn surface(in: SurfaceInput) -> Surface {\n    var s = standard_surface(in);\n    let mask = textureSample(mask, surface_sampler, in.uv).r;\n    s.base_color = s.base_color * material.tint.rgb * mask;\n    s.roughness = material.roughness;\n    return s;\n}\n";

fn layout() -> Layout {
    let uniforms: BTreeMap<String, Uniform> = [
        ("roughness".to_owned(), Uniform::Float(0.5)),
        ("tint".to_owned(), Uniform::Color((1.0, 0.5, 0.25, 1.0))),
    ]
    .into_iter()
    .collect();
    Layout::new(&uniforms).expect("fits")
}

fn checked(
    source: &str,
    textures: &[&str],
    lighting: SurfaceLighting,
) -> Result<SurfaceShader, String> {
    let composed = compose_surface(source, &layout(), textures, lighting)?;
    SurfaceShader::new(composed, textures.len())
}

fn module(source: &str) -> naga::Module {
    naga::front::wgsl::parse_str(source).expect("parses")
}

#[test]
fn a_custom_surface_composes_valid_from_the_standard_one() {
    let shader = checked(TINTED, &["mask"], SurfaceLighting::Lit).expect("valid");
    assert!(shader.source().contains("var mask: texture_2d<f32>"));
    assert!(shader.source().contains("return light_surface(s, in);"));
    let module = module(shader.source());
    for entry in SURFACE_ENTRIES {
        assert!(
            module.entry_points.iter().any(|e| e.name == entry),
            "{entry}"
        );
    }
    // The depth-only cut-out writes no colour.
    let cutout = module
        .entry_points
        .iter()
        .find(|e| e.name == "fs_cutout")
        .expect("emitted");
    assert!(cutout.function.result.is_none());
    let unlit = checked(TINTED, &["mask"], SurfaceLighting::Unlit).expect("valid");
    assert!(unlit.source().contains("return unlit_surface(s);"));
    assert_ne!(unlit.revision(), shader.revision(), "another shader");
}

#[test]
fn the_same_source_is_the_same_revision() {
    let a = checked(TINTED, &["mask"], SurfaceLighting::Lit).expect("valid");
    let b = checked(TINTED, &["mask"], SurfaceLighting::Lit).expect("valid");
    assert_eq!(a.revision(), b.revision());
    assert_eq!(a, b);
    let edited = checked(
        &TINTED.replace("mask;", "mask * 0.5;"),
        &["mask"],
        SurfaceLighting::Lit,
    )
    .expect("valid");
    assert_ne!(edited.revision(), a.revision());
    let standard = SurfaceShader::standard();
    assert_eq!(standard.source(), standard_source());
    assert!(
        Arc::ptr_eq(&standard, &SurfaceShader::standard()),
        "once a process"
    );
}

#[test]
fn a_broken_surface_says_what_and_where() {
    let error = checked(
        "fn surface(in: SurfaceInput) -> Surface { var s = standard_surface(in); s.alpha = material.nope; return s; }",
        &[],
        SurfaceLighting::Lit,
    )
    .expect_err("no such field");
    assert!(error.contains("nope"), "{error}");
    let missing = checked("fn other() {}", &[], SurfaceLighting::Lit).expect_err("no surface()");
    assert!(missing.contains("surface"), "{missing}");
    let wrong = checked(
        "fn surface(in: SurfaceInput) -> vec4<f32> { return vec4<f32>(1.0); }",
        &[],
        SurfaceLighting::Lit,
    );
    assert!(wrong.is_err(), "returns the wrong type");
    // A texture it reads but did not name.
    assert!(checked(TINTED, &[], SurfaceLighting::Lit).is_err());
}

#[test]
fn a_fifth_texture_or_a_bad_name_is_refused() {
    let five = ["a", "b", "c", "d", "e"];
    let error =
        compose_surface(TINTED, &layout(), &five, SurfaceLighting::Lit).expect_err("five textures");
    assert!(error.contains("at most 4"), "{error}");
    assert!(compose_surface(TINTED, &layout(), &["not a name"], SurfaceLighting::Lit).is_err());
    assert!(compose_surface(TINTED, &layout(), &["__x"], SurfaceLighting::Lit).is_err());
    // Four is the budget, and fits.
    let four = "fn surface(in: SurfaceInput) -> Surface {\n    var s = standard_surface(in);\n    let uv = in.uv;\n    s.base_color = textureSample(a, surface_sampler, uv).rgb + textureSample(b, surface_sampler, uv).rgb + textureSample(c, surface_sampler, uv).rgb + textureSample(d, surface_sampler, uv).rgb;\n    return s;\n}\n";
    let shader = checked(four, &["a", "b", "c", "d"], SurfaceLighting::Lit).expect("four fit");
    let module = module(shader.source());
    let textures = module
        .global_variables
        .iter()
        .filter(|(_, g)| matches!(module.types[g.ty].inner, naga::TypeInner::Image { .. }))
        .count() as u32;
    // Five standard and four of its own, of WebGL2's 16 — and room for
    // 140's shadow array and 141's cube and table within the plan's 12.
    assert_eq!(textures, SURFACE_TEXTURES + CUSTOM_TEXTURES as u32);
    assert!(textures + 3 <= 12);
    assert!(
        textures <= wgpu::Limits::downlevel_webgl2_defaults().max_sampled_textures_per_shader_stage
    );
}

#[test]
fn a_material_binding_its_own_resources_is_refused() {
    let own = "@group(1) @binding(12) var extra: texture_2d<f32>;\nfn surface(in: SurfaceInput) -> Surface { return standard_surface(in); }\n";
    let error = checked(own, &[], SurfaceLighting::Lit).expect_err("its own binding");
    assert!(
        error.contains("extra") && error.contains("engine's"),
        "{error}"
    );
    // One the layout has, but the composer did not declare for it: a
    // custom slot past its named textures.
    let unnamed = "@group(1) @binding(9) var sneaky: texture_2d<f32>;\nfn surface(in: SurfaceInput) -> Surface { return standard_surface(in); }\n";
    assert!(checked(unnamed, &["mask"], SurfaceLighting::Lit).is_err());
}

#[test]
fn the_uniform_block_is_a_multiple_of_16_bytes_for_webgl2() {
    // WebGL2 refuses a uniform binding whose type is not a multiple of 16
    // bytes: a lone f32 or vec2 is 4 or 8 unless the struct is aligned.
    for uniforms in [
        vec![("speed".to_owned(), Uniform::Vec2((0.25, 0.0)))],
        vec![("amount".to_owned(), Uniform::Float(0.5))],
        vec![
            ("a".to_owned(), Uniform::Float(0.5)),
            ("b".to_owned(), Uniform::Vec2((0.0, 0.0))),
            ("c".to_owned(), Uniform::Float(0.0)),
        ],
        vec![],
    ] {
        let uniforms: BTreeMap<String, Uniform> = uniforms.into_iter().collect();
        let layout = Layout::new(&uniforms).expect("fits");
        let source = "fn surface(in: SurfaceInput) -> Surface { return standard_surface(in); }\n";
        let composed =
            compose_surface(source, &layout, &[], SurfaceLighting::Lit).expect("composes");
        let shader = SurfaceShader::new(composed, 0).unwrap_or_else(|e| panic!("{e}"));
        let module = module(shader.source());
        let (_, material) = module
            .global_variables
            .iter()
            .find(|(_, g)| g.name.as_deref() == Some("material"))
            .expect("declared");
        let naga::TypeInner::Struct { span, .. } = module.types[material.ty].inner else {
            panic!("a struct");
        };
        assert_eq!(span as usize, layout.size(), "{uniforms:?}");
        assert_eq!(span % 16, 0);
    }
}
