//! What the custom surface tests share: the shipped shaders, a shading
//! made from a source, custom texture slots, a row of pixels, a shipped
//! texture.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::surface::*;
use super::*;
use voltra_render::glam::{Quat, Vec3};
use voltra_render::material::{Layout, Uniform};
use voltra_render::render3d::{
    compose_surface, CustomImage, CustomShading, Lighting3D, Look3D, Mesh3D, SurfaceLighting,
    SurfaceMaterial3D, SurfaceShader, SurfaceTextures, TextureSpace, CUSTOM_TEXTURES,
};
use voltra_render::Texture;

pub const WORLD_TINT: &str = include_str!("../../../../assets/materials/world_tint.wgsl");
pub const SCROLL: &str = include_str!("../../../../assets/materials/scroll.wgsl");
pub const DISSOLVE: &str = include_str!("../../../../assets/materials/dissolve.wgsl");

/// `source`'s shading with `uniforms` at their defaults, sampling the
/// named `textures`.
pub fn shading(
    source: &str,
    uniforms: &[(&str, Uniform)],
    textures: &[&str],
    lighting: SurfaceLighting,
) -> CustomShading {
    let uniforms: BTreeMap<String, Uniform> = uniforms
        .iter()
        .map(|(n, u)| ((*n).to_owned(), *u))
        .collect();
    let layout = Layout::new(&uniforms).expect("fits");
    let composed = compose_surface(source, &layout, textures, lighting).expect("composes");
    let shader = SurfaceShader::new(composed, textures.len()).unwrap_or_else(|e| panic!("{e}"));
    CustomShading {
        shader: Arc::new(shader),
        block: layout.block(&BTreeMap::new()),
    }
}

pub fn with_custom<'a>(images: &[(&'a Texture, TextureSpace)]) -> SurfaceTextures<'a> {
    let mut custom = [None; CUSTOM_TEXTURES];
    for (slot, (texture, space)) in custom.iter_mut().zip(images) {
        *slot = Some(CustomImage {
            texture,
            space: *space,
        });
    }
    SurfaceTextures {
        custom,
        ..SurfaceTextures::NONE
    }
}

/// A square `width` × `span` metres wide, moved `shift` along x, under a
/// `width` × 1 columns view: filling it while `span` covers the shift.
pub fn row(
    gpu: &mut Gpu,
    width: u32,
    material: &SurfaceMaterial3D,
    (shift, span): (f32, f32),
    time: f32,
) -> Vec<[u8; 4]> {
    let square = Mesh3D::new(&gpu.device, "square", &square(0.5)).expect("a square");
    picture_at(
        gpu,
        (width, 1),
        columns_view(width),
        Lighting3D::DARK,
        &[Draw {
            mesh: &square,
            material,
            instances: vec![scaled(
                Vec3::new(shift, 0.0, -5.0),
                Quat::IDENTITY,
                Vec3::new(width as f32 * span, 1.0, 1.0),
                WHITE,
            )],
        }],
        time,
        &Look3D::NEUTRAL,
    )
}

/// A texture shipped in `assets/textures/`.
pub fn shipped(gpu: &Gpu, name: &str) -> Texture {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/textures")
        .join(name);
    let image = image::open(&path).expect("the shipped texture").to_rgba8();
    Texture::from_rgba8(
        &gpu.device,
        &gpu.queue,
        name,
        image.as_raw(),
        image.width(),
        image.height(),
        voltra_render::Filter::Linear,
    )
    .expect("uploaded")
}
