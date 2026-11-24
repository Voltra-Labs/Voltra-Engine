//! Materials in the batch: a run splits where a material or its values
//! change, and nowhere else.

use voltra_assets::{AssetPath, Materials};
use voltra_render::glam::Mat3;
use voltra_render::material::Uniform;
use voltra_testkit::scratch_root;

use super::SpriteBatch;
use crate::sprite::sheets::Sheets;
use crate::{Sprite, SpriteMaterial};

fn flash_store() -> (Materials, AssetPath) {
    let root = scratch_root();
    std::fs::create_dir_all(root.join("materials")).expect("folder");
    std::fs::write(
        root.join("materials/flash.wgsl"),
        "fn fragment(in: Fragment) -> vec4<f32> { return sprite_color(in) + vec4<f32>(material.flash); }",
    )
    .expect("shader");
    std::fs::write(
        root.join("materials/flash.material.ron"),
        r#"(version: 1, shader: "materials/flash.wgsl", uniforms: {"flash": Float(0.0)})"#,
    )
    .expect("material");
    (
        Materials::new(root),
        AssetPath::new("materials/flash.material.ron").expect("path"),
    )
}

fn sprite(material: Option<(&mut Materials, &AssetPath, f32)>) -> Sprite {
    let mut sprite = Sprite::default();
    if let Some((store, path, flash)) = material {
        let mut slot = SpriteMaterial::new(path.clone());
        slot.set("flash", Uniform::Float(flash));
        slot.resolve(store);
        sprite.material = Some(slot);
    }
    sprite
}

#[test]
fn runs_split_only_where_the_material_or_its_values_change() {
    let (mut store, path) = flash_store();
    let sprites = [
        sprite(None),
        sprite(Some((&mut store, &path, 1.0))),
        sprite(Some((&mut store, &path, 1.0))),
        sprite(Some((&mut store, &path, 0.5))),
        sprite(None),
        sprite(None),
    ];
    let sheets = Sheets {
        materials: Some(&store),
        ..Sheets::default()
    };
    let mut batch = SpriteBatch::default();
    for s in &sprites {
        batch.push(Mat3::IDENTITY, s, sheets);
    }
    let runs: Vec<(Option<u32>, u32)> = batch
        .ranges
        .iter()
        .map(|r| match &r.primitives {
            voltra_render::Primitives::Quads(quads) => (r.material, quads.end - quads.start),
            voltra_render::Primitives::Triangles(indices) => {
                (r.material, (indices.end - indices.start) / 6)
            }
        })
        .collect();
    assert_eq!(runs, vec![(None, 1), (Some(0), 2), (Some(1), 1), (None, 2)]);
    assert_eq!(batch.materials.len(), 2, "one use per set of values");
}

#[test]
fn without_a_store_or_with_none_it_batches_as_before() {
    let (mut store, path) = flash_store();
    let with = sprite(Some((&mut store, &path, 1.0)));
    let mut batch = SpriteBatch::default();
    for s in [&with, &Sprite::default(), &with] {
        batch.push(Mat3::IDENTITY, s, Sheets::default());
    }
    assert_eq!(
        batch.ranges.len(),
        1,
        "no store: every sprite plain, one run"
    );
    assert!(batch.materials.is_empty());
}
