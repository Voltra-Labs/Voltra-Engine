//! The frame's surface materials on the GPU: one per material handle the
//! draw list names, and the default white standard, each remade when its
//! material — its numbers, its shader, its uniforms — or one of its
//! textures changes.

use std::collections::HashMap;

use voltra_assets::{Handle, Material, Materials, SurfaceSource, SurfaceTexturePaths, Textures};
use voltra_render::render3d::{
    CustomImage, StandardSurface, SurfaceMaterial3D, SurfaceTextures, CUSTOM_TEXTURES,
};
use voltra_render::Renderer;
use voltra_scene::Draw3D;

/// What the 3D draws bind as group 1, kept from frame to frame.
#[derive(Debug, Default)]
pub(in crate::app) struct Surfaces {
    /// The default white standard: a mesh with no material, or one whose
    /// material cannot be drawn.
    default: Option<SurfaceMaterial3D>,
    /// By material handle. `Materials` never removes, so this is bounded
    /// by the materials a session loads.
    by_material: HashMap<Handle<Material>, SurfaceMaterial3D>,
}

impl Surfaces {
    /// Makes, or remakes when stale, the GPU material of every draw in
    /// `draws`: the surface `materials` holds for its handle, with the
    /// images `textures` holds for its paths.
    pub fn prepare(
        &mut self,
        renderer: &mut Renderer,
        materials: Option<&Materials>,
        textures: &Textures,
        draws: &[Draw3D],
    ) {
        if self.default.is_none() {
            self.default =
                Some(renderer.surface_material(&StandardSurface::DEFAULT, &SurfaceTextures::NONE));
        }
        let Some(materials) = materials else {
            return;
        };
        for handle in draws.iter().filter_map(|d| d.material) {
            let Some(source) = materials.get(handle).and_then(Material::surface) else {
                continue;
            };
            let images = images(textures, &source.textures);
            let layouts = renderer.surface_layouts().clone();
            let custom = source.custom.as_ref().map(|c| c.shading());
            let current = self.by_material.get(&handle).is_some_and(|gpu| {
                gpu.is_current(&layouts, &source.standard, &images, custom.as_ref())
            });
            if !current {
                let gpu = gpu_material(renderer, source, &images);
                self.by_material.insert(handle, gpu);
            }
        }
    }

    /// The GPU material `material` draws with: its own when prepared, the
    /// default otherwise. `None` only before the first
    /// [`prepare`](Self::prepare).
    pub fn get(&self, material: Option<Handle<Material>>) -> Option<&SurfaceMaterial3D> {
        material
            .and_then(|handle| self.by_material.get(&handle))
            .or(self.default.as_ref())
    }
}

/// `source` with `images`, made for `renderer`: through its own shader
/// and uniforms when it is a custom surface, the standard's otherwise.
pub(in crate::app) fn gpu_material(
    renderer: &mut Renderer,
    source: &SurfaceSource,
    images: &SurfaceTextures<'_>,
) -> SurfaceMaterial3D {
    let custom = source.custom.as_ref().map(|c| c.shading());
    renderer.custom_surface_material(&source.standard, images, custom.as_ref())
}

/// The images `paths` names, as loaded in `textures`: a slot whose image
/// is not loaded yet is empty (white), and so is one naming a render texture (a
/// 3D surface showing a camera's picture is not drawn yet; the frame's
/// resolve warns once). A data slot (metallic-roughness, normal,
/// occlusion) whose file would not load is empty too — the checker's
/// pixels as normals would be nonsense — while a colour slot shows the
/// checker, as a sprite's missing image does.
pub(in crate::app) fn images<'a>(
    textures: &'a Textures,
    paths: &SurfaceTexturePaths,
) -> SurfaceTextures<'a> {
    let loaded = |path: &Option<voltra_assets::AssetPath>| {
        let handle = textures.by_path_handle(path.as_ref()?)?;
        textures.render_texture(handle).is_none().then_some(handle)
    };
    let colour = |path: &Option<voltra_assets::AssetPath>| textures.try_get(loaded(path)?);
    let data = |path: &Option<voltra_assets::AssetPath>| {
        let handle = loaded(path)?;
        if textures.is_broken(handle) {
            return None;
        }
        textures.try_get(handle)
    };
    let mut custom = [None; CUSTOM_TEXTURES];
    for (slot, texture) in custom.iter_mut().zip(&paths.custom) {
        let path = texture.path.clone();
        let image = match texture.space {
            voltra_render::render3d::TextureSpace::Color => colour(&path),
            voltra_render::render3d::TextureSpace::Data => data(&path),
        };
        *slot = image.map(|texture_image| CustomImage {
            texture: texture_image,
            space: texture.space,
        });
    }
    SurfaceTextures {
        base_color: colour(&paths.base_color),
        metallic_roughness: data(&paths.metallic_roughness),
        normal: data(&paths.normal),
        occlusion: data(&paths.occlusion),
        emissive: colour(&paths.emissive),
        custom,
    }
}
