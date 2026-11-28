use voltra_testkit::scratch_root;

use super::*;
use crate::material::MATERIAL_VERSION;

const FLASH: &str = "fn fragment(in: Fragment) -> vec4<f32> {\n    let base = sprite_color(in);\n    return vec4<f32>(mix(base.rgb, material.tint.rgb, material.amount), base.a);\n}\n";

fn write(root: &std::path::Path, name: &str, text: &str) {
    let path = root.join(name);
    std::fs::create_dir_all(path.parent().expect("a folder")).expect("folders");
    std::fs::write(path, text).expect("written");
}

fn flash_root() -> voltra_testkit::Scratch {
    let root = scratch_root();
    write(&root, "materials/flash.wgsl", FLASH);
    write(
        &root,
        "materials/flash.material.ron",
        &format!(
            r#"(version: {MATERIAL_VERSION}, shader: "materials/flash.wgsl", uniforms: {{"amount": Float(0.0), "tint": Color((1.0, 1.0, 1.0, 1.0))}})"#
        ),
    );
    root
}

fn path(raw: &str) -> AssetPath {
    AssetPath::new(raw).expect("a path")
}

#[test]
fn a_material_loads_composed_and_laid_out() {
    let root = flash_root();
    let mut materials = Materials::new(&root);
    let handle = materials.load(&path("materials/flash.material.ron"));
    let material = materials.get(handle).expect("loaded");
    assert!(material.is_usable(), "{:?}", material.error);
    assert!(material.source.contains("fn fs_main"));
    assert_eq!(material.layout.as_ref().map(|l| l.size()), Some(32));
    assert_eq!(
        handle,
        materials.load(&path("materials/flash.material.ron")),
        "once"
    );
}

#[test]
fn a_broken_shader_is_a_material_with_an_error() {
    let root = flash_root();
    write(
        &root,
        "materials/flash.wgsl",
        "fn fragment(in: Fragment) -> vec4<f32> { return 1; }",
    );
    let mut materials = Materials::new(&root);
    let handle = materials.load(&path("materials/flash.material.ron"));
    let material = materials.get(handle).expect("a handle still");
    assert!(!material.is_usable());
    assert!(material.error.as_deref().is_some_and(|e| !e.is_empty()));
    let missing = materials.load(&path("materials/none.material.ron"));
    assert!(!materials.get(missing).expect("a handle").is_usable());
}

#[test]
fn editing_the_shader_reloads_the_material_that_names_it() {
    let root = flash_root();
    let mut materials = Materials::new(&root);
    let handle = materials.load(&path("materials/flash.material.ron"));
    let before = materials.get(handle).expect("loaded").revision;
    write(
        &root,
        "materials/flash.wgsl",
        "fn fragment(in: Fragment) -> vec4<f32> { return sprite_color(in) * material.amount; }",
    );
    assert!(materials.reload(&path("materials/flash.wgsl")));
    let after = materials.get(handle).expect("loaded");
    assert!(after.revision > before && after.is_usable());
    assert!(after.source.contains("* material.amount"));
    assert!(!materials.reload(&path("sprites/x.png")));
}

#[test]
fn a_0_4_sprite_material_file_loads_as_a_sprite_material() {
    // The shipped hit flash, byte for byte as 0.4 wrote it: no domain.
    let root = scratch_root();
    voltra_testkit::copy_workspace_asset(&root, "materials/hit_flash.material.ron");
    voltra_testkit::copy_workspace_asset(&root, "materials/hit_flash.wgsl");
    let text = std::fs::read_to_string(root.join("materials/hit_flash.material.ron"))
        .expect("the shipped file");
    assert!(!text.contains("domain"), "a file written before 3D");
    let mut materials = Materials::new(&root);
    let handle = materials.load(&path("materials/hit_flash.material.ron"));
    let material = materials.get(handle).expect("loaded");
    assert_eq!(material.domain, crate::MaterialDomain::Sprite);
    assert!(material.is_usable(), "{:?}", material.error);
    assert!(material.surface().is_none());
}

#[test]
fn a_surface_material_loads_its_standard_block() {
    let root = scratch_root();
    write(
        &root,
        "materials/gold.material.ron",
        r#"(version: 1, domain: Surface, standard: (
            base_color: (1.0, 0.77, 0.34, 1.0), metallic: 1.0, roughness: 0.3,
            normal_texture: "textures/n.png", alpha: Mask(0.4), double_sided: true,
            uv_transform: (offset: (0.25, 0.0)),
        ))"#,
    );
    let mut materials = Materials::new(&root);
    let handle = materials.load(&path("materials/gold.material.ron"));
    let material = materials.get(handle).expect("loaded");
    assert_eq!(material.domain, crate::MaterialDomain::Surface);
    assert!(!material.is_usable(), "not a sprite material");
    let surface = material.surface().expect("a surface");
    assert_eq!(surface.standard.base_color, [1.0, 0.77, 0.34, 1.0]);
    assert_eq!(surface.standard.metallic, 1.0);
    assert_eq!(
        surface.standard.alpha,
        voltra_render::render3d::AlphaMode::Mask(0.4)
    );
    assert!(surface.standard.double_sided);
    assert_eq!(surface.standard.uv_transform.offset, [0.25, 0.0]);
    assert_eq!(surface.standard.uv_transform.scale, [1.0, 1.0]);
    // Left out: the defaults.
    assert_eq!(surface.standard.normal_scale, 1.0);
    assert_eq!(surface.textures.normal, Some(path("textures/n.png")));
    assert_eq!(surface.textures.base_color, None);
    assert_eq!(surface.textures.iter().count(), 1);
}

#[test]
fn a_surface_material_with_a_bad_texture_path_or_a_shader_says_why() {
    let root = scratch_root();
    write(
        &root,
        "materials/bad.material.ron",
        r#"(version: 1, domain: Surface, standard: (base_color_texture: "../outside.png"))"#,
    );
    write(
        &root,
        "materials/shaded.material.ron",
        r#"(version: 1, domain: Surface, shader: "materials/x.wgsl")"#,
    );
    let mut materials = Materials::new(&root);
    for (name, says) in [("bad", "base_color_texture"), ("shaded", "shader")] {
        let handle = materials.load(&path(&format!("materials/{name}.material.ron")));
        let material = materials.get(handle).expect("a handle still");
        assert!(material.surface().is_none());
        let error = material.error.as_deref().unwrap_or_default();
        assert!(error.contains(says), "{name}: {error}");
    }
}

#[test]
fn a_surface_material_written_by_the_editor_reads_back_the_same() {
    let standard = crate::StandardFile {
        base_color: [0.2, 0.4, 0.6, 1.0],
        roughness: 0.8,
        emissive: [1.0, 0.5, 0.0],
        emissive_strength: 3.0,
        normal_texture: "textures/n.png".to_owned(),
        alpha: voltra_render::render3d::AlphaMode::Blend,
        ..crate::StandardFile::default()
    };
    let file = MaterialFile::surface(standard);
    let text = file.to_ron().expect("written");
    assert!(text.contains("domain: Surface"), "{text}");
    assert!(
        !text.contains("shader"),
        "nothing a surface does not use: {text}"
    );
    assert!(
        !text.contains("uv_transform"),
        "an identity is left out: {text}"
    );
    let back: MaterialFile = ron::from_str(&text).expect("read back");
    assert_eq!(back, file);
}
