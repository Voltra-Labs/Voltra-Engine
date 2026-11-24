use voltra_testkit::scratch_root;

use super::*;
use crate::material::MATERIAL_VERSION;

const FLASH: &str = "fn fragment(in: Fragment) -> vec4<f32> {\n    let base = sprite_color(in);\n    return vec4<f32>(mix(base.rgb, material.tint.rgb, material.amount), base.a);\n}\n";

fn write(root: &std::path::Path, name: &str, text: &str) {
    let path = root.join(name);
    std::fs::create_dir_all(path.parent().expect("a folder")).expect("folders");
    std::fs::write(path, text).expect("written");
}

fn flash_root() -> std::path::PathBuf {
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
    let mut materials = Materials::new(flash_root());
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
