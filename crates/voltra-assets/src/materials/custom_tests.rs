//! Custom surface materials in the store: composed and checked when read,
//! a broken one drawn as its standard block with its error, reloaded when
//! its `.wgsl` changes.

use voltra_render::material::Uniform;
use voltra_render::render3d::{SurfaceLighting, TextureSpace};
use voltra_testkit::scratch_root;

use super::*;
use crate::material::CustomTextureFile;

fn write(root: &std::path::Path, name: &str, text: &str) {
    let path = root.join(name);
    std::fs::create_dir_all(path.parent().expect("a folder")).expect("folders");
    std::fs::write(path, text).expect("written");
}

fn path(raw: &str) -> AssetPath {
    AssetPath::new(raw).expect("a path")
}

/// The shipped dissolve, its shader and its material file.
fn dissolve_root() -> voltra_testkit::Scratch {
    let root = scratch_root();
    voltra_testkit::copy_workspace_asset(&root, "materials/dissolve.material.ron");
    voltra_testkit::copy_workspace_asset(&root, "materials/dissolve.wgsl");
    root
}

const DISSOLVE: &str = "materials/dissolve.material.ron";

#[test]
fn the_shipped_dissolve_loads_composed_with_its_uniforms_and_texture() {
    let root = dissolve_root();
    let mut materials = Materials::new(&root);
    let handle = materials.load(&path(DISSOLVE));
    let material = materials.get(handle).expect("loaded");
    assert_eq!(material.error, None);
    assert!(material.warnings.is_empty(), "{:?}", material.warnings);
    assert_eq!(material.shader, Some(path("materials/dissolve.wgsl")));
    let surface = material.surface().expect("a surface");
    let custom = surface.custom.as_ref().expect("its own shading");
    assert!(custom
        .shader
        .source()
        .contains("let noise = textureSample(noise"));
    assert_eq!(custom.lighting, SurfaceLighting::Lit);
    assert_eq!(custom.uniforms.get("amount"), Some(&Uniform::Float(0.45)));
    // amount, edge (f32s), then edge_color at 16: 32 bytes.
    assert_eq!(custom.layout.size(), 32);
    let block = custom.block();
    assert_eq!(
        f32::from_le_bytes(block[0..4].try_into().expect("four")),
        0.45
    );
    assert_eq!(
        f32::from_le_bytes(block[16..20].try_into().expect("four")),
        4.0
    );
    // Its texture is among the images the frame loads, after the standard
    // five, read as data.
    let textures: Vec<_> = surface.textures.iter().collect();
    assert_eq!(textures, vec![&path("textures/noise.png")]);
    assert_eq!(surface.textures.custom[0].name, "noise");
    assert_eq!(surface.textures.custom[0].space, TextureSpace::Data);
    assert_eq!(
        surface.standard.alpha,
        voltra_render::render3d::AlphaMode::Mask(0.5)
    );
}

#[test]
fn a_broken_shader_draws_the_standard_block_with_its_error() {
    let root = dissolve_root();
    write(
        &root,
        "materials/dissolve.wgsl",
        "fn surface(in: SurfaceInput) -> Surface {\n    var s = standard_surface(in);\n    s.alpha = material.nope;\n    return s;\n}\n",
    );
    let mut materials = Materials::new(&root);
    let handle = materials.load(&path(DISSOLVE));
    let material = materials.get(handle).expect("a handle still");
    let error = material.error.as_deref().expect("an error");
    assert!(error.contains("nope"), "says what: {error}");
    let surface = material.surface().expect("drawn as its standard block");
    assert!(surface.custom.is_none());
    assert_eq!(surface.standard.base_color, [0.6, 0.62, 0.66, 1.0]);
    // A shader binding its own resource is refused the same way.
    write(
        &root,
        "materials/dissolve.wgsl",
        "@group(2) @binding(0) var<uniform> mine: vec4<f32>;\nfn surface(in: SurfaceInput) -> Surface { return standard_surface(in); }\n",
    );
    assert!(materials.reload(&path("materials/dissolve.wgsl")));
    let error = materials.get(handle).and_then(|m| m.error.clone());
    assert!(error.is_some_and(|e| e.contains("mine")));
}

#[test]
fn editing_the_wgsl_reloads_the_material_under_its_handle() {
    let root = dissolve_root();
    let mut materials = Materials::new(&root);
    let handle = materials.load(&path(DISSOLVE));
    let before = materials.get(handle).expect("loaded").clone();
    let shader = |m: &Material| {
        m.surface()
            .and_then(|s| s.custom.as_ref())
            .map(|c| c.shader.revision())
    };
    // Broken, then fixed, each by writing the file and its watch's reload.
    write(&root, "materials/dissolve.wgsl", "fn surface( {");
    assert!(materials.reload(&path("materials/dissolve.wgsl")));
    let broken = materials.get(handle).expect("the same handle").clone();
    assert!(broken.error.is_some() && broken.revision > before.revision);
    assert_eq!(shader(&broken), None);
    write(
        &root,
        "materials/dissolve.wgsl",
        "fn surface(in: SurfaceInput) -> Surface {\n    var s = standard_surface(in);\n    s.base_color = vec3<f32>(material.amount);\n    return s;\n}\n",
    );
    assert!(materials.reload(&path("materials/dissolve.wgsl")));
    let fixed = materials.get(handle).expect("the same handle");
    assert_eq!(fixed.error, None);
    assert!(fixed.revision > broken.revision);
    assert!(shader(fixed).is_some() && shader(fixed) != shader(&before));
    assert!(!materials.reload(&path("materials/other.wgsl")));
}

#[test]
fn a_missing_shader_file_reloads_once_written() {
    let root = scratch_root();
    write(
        &root,
        "materials/late.material.ron",
        r#"(version: 1, domain: Surface, shader: "materials/late.wgsl")"#,
    );
    let mut materials = Materials::new(&root);
    let handle = materials.load(&path("materials/late.material.ron"));
    assert!(materials.get(handle).is_some_and(|m| m.error.is_some()));
    write(
        &root,
        "materials/late.wgsl",
        "fn surface(in: SurfaceInput) -> Surface { return standard_surface(in); }\n",
    );
    assert!(
        materials.reload(&path("materials/late.wgsl")),
        "watched though missing"
    );
    assert_eq!(materials.get(handle).and_then(|m| m.error.clone()), None);
}

#[test]
fn a_fifth_texture_is_refused_and_the_standard_block_drawn() {
    let root = scratch_root();
    write(
        &root,
        "materials/five.wgsl",
        "fn surface(in: SurfaceInput) -> Surface { return standard_surface(in); }\n",
    );
    write(
        &root,
        "materials/five.material.ron",
        r#"(version: 1, domain: Surface, shader: "materials/five.wgsl", textures: {
            "a": Data("t/a.png"), "b": Data("t/b.png"), "c": Data("t/c.png"),
            "d": Data("t/d.png"), "e": Color("t/e.png"),
        })"#,
    );
    let mut materials = Materials::new(&root);
    let handle = materials.load(&path("materials/five.material.ron"));
    let material = materials.get(handle).expect("a handle");
    let error = material.error.as_deref().unwrap_or_default();
    assert!(
        error.contains("5 textures") && error.contains("at most 4"),
        "{error}"
    );
    let surface = material.surface().expect("the standard block");
    assert!(surface.custom.is_none() && surface.textures.custom.is_empty());
}

#[test]
fn blocks_without_a_shader_are_warned_about() {
    let root = scratch_root();
    write(
        &root,
        "materials/plain.material.ron",
        r#"(version: 1, domain: Surface, lighting: Unlit, textures: {"n": Data("t/n.png")})"#,
    );
    write(
        &root,
        "materials/flash.wgsl",
        "fn fragment(in: Fragment) -> vec4<f32> { return sprite_color(in); }\n",
    );
    write(
        &root,
        "materials/sprite.material.ron",
        r#"(version: 1, shader: "materials/flash.wgsl", textures: {"n": Data("t/n.png")})"#,
    );
    let mut materials = Materials::new(&root);
    let plain = materials.load(&path("materials/plain.material.ron"));
    let warnings = &materials.get(plain).expect("loaded").warnings;
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(materials.get(plain).is_some_and(|m| m.error.is_none()));
    let sprite = materials.load(&path("materials/sprite.material.ron"));
    let warnings = &materials.get(sprite).expect("loaded").warnings;
    assert!(
        warnings.iter().any(|w| w.contains("textures")),
        "{warnings:?}"
    );
}

#[test]
fn a_custom_surface_written_by_the_editor_reads_back_the_same() {
    let mut file = MaterialFile::surface(crate::StandardFile::default());
    file.shader = "materials/x.wgsl".to_owned();
    file.uniforms
        .insert("amount".to_owned(), Uniform::Float(0.25));
    file.textures.insert(
        "noise".to_owned(),
        CustomTextureFile::Data("textures/noise.png".to_owned()),
    );
    file.lighting = SurfaceLighting::Unlit;
    let text = file.to_ron().expect("written");
    assert!(text.contains("Data(\"textures/noise.png\")"), "{text}");
    assert!(text.contains("lighting: Unlit"), "{text}");
    let back: MaterialFile = ron::from_str(&text).expect("read back");
    assert_eq!(back, file);
    // Lit and no textures are left out.
    let plain = MaterialFile::surface(crate::StandardFile::default());
    let text = plain.to_ron().expect("written");
    assert!(
        !text.contains("lighting") && !text.contains("textures"),
        "{text}"
    );
    assert_eq!(
        file.surface_texture_paths().expect("paths").iter().count(),
        1
    );
}

#[test]
fn a_texture_named_with_no_image_yet_is_a_white_slot_not_an_error() {
    // What the inspector's Add makes: the name, no path.
    let root = scratch_root();
    write(
        &root,
        "materials/new.wgsl",
        "fn surface(in: SurfaceInput) -> Surface {\n    var s = standard_surface(in);\n    s.base_color = textureSample(mask, surface_sampler, in.uv).rgb;\n    return s;\n}\n",
    );
    write(
        &root,
        "materials/new.material.ron",
        r#"(version: 1, domain: Surface, shader: "materials/new.wgsl", textures: {"mask": Data("")})"#,
    );
    let mut materials = Materials::new(&root);
    let handle = materials.load(&path("materials/new.material.ron"));
    let material = materials.get(handle).expect("loaded");
    assert_eq!(material.error, None);
    let surface = material.surface().expect("a surface");
    assert!(surface.custom.is_some(), "its own shading");
    assert_eq!(surface.textures.custom.len(), 1, "the slot kept, in order");
    assert_eq!(surface.textures.custom[0].path, None);
    assert_eq!(surface.textures.iter().count(), 0, "nothing to load");
}
