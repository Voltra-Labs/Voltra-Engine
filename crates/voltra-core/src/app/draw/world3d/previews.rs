//! Surface materials drawn on a lit sphere, offscreen, for the editor to
//! show: the inspector's live preview and the asset browser's thumbnails.
//!
//! A panel asks during the layout ([`UiFrame::surface_preview`]); after
//! the layout, every preview asked for is drawn — again only when what it
//! shows changed — into a target egui samples. The first frame a preview
//! is asked for it has no id yet; from the next, it shows what this frame
//! drew, as the viewport does.
//!
//! [`UiFrame::surface_preview`]: crate::UiFrame::surface_preview

use std::collections::HashMap;

use voltra_assets::{BuiltinMesh, Meshes, SurfaceSource, Textures};
use voltra_render::glam::{Affine3A, Quat, Vec3};
use voltra_render::render3d::{
    DirectionalLight, Instance3D, Lighting3D, Look3D, MeshDraw3D, Scene3D, View3D, Winding,
};
use voltra_render::{wgpu, Filter, Layer, RenderTarget, Renderer};

use super::surfaces::images;
use crate::ui::TextureId;

/// The largest preview drawn, in pixels a side.
pub const MAX_PREVIEW: u32 = 512;

/// Draws a preview is kept for without being asked for: a thumbnail
/// scrolled away, a material no longer selected. Then its target and its
/// egui id are let go.
pub const KEEP_DRAWS: u64 = 120;

/// One preview a panel asked for this frame.
#[derive(Debug, Clone, PartialEq)]
struct Request {
    key: String,
    size: u32,
    source: SurfaceSource,
}

/// A preview's target, its egui id and what it last drew.
struct Shown {
    target: RenderTarget,
    id: TextureId,
    /// What it last drew, and through which texture views: a texture
    /// reloaded under its path has new ones.
    drawn: Option<(SurfaceSource, Vec<wgpu::TextureView>, wgpu::TextureFormat)>,
    /// The last [`SurfacePreviews::draw`] it was asked for before.
    asked: u64,
}

/// The previews asked for and those drawn.
#[derive(Default)]
pub(in crate::app) struct SurfacePreviews {
    requests: Vec<Request>,
    shown: HashMap<(String, u32), Shown>,
    /// Counts the calls to [`draw`](Self::draw).
    draws: u64,
}

impl std::fmt::Debug for SurfacePreviews {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SurfacePreviews")
            .field("requests", &self.requests.len())
            .field("shown", &self.shown.len())
            .finish()
    }
}

impl SurfacePreviews {
    /// Asks for `source` drawn under `key` at `size` pixels a side this
    /// frame; the id to show it by, from the second frame it is asked.
    pub fn ask(&mut self, key: &str, size: u32, source: &SurfaceSource) -> Option<TextureId> {
        let size = size.clamp(1, MAX_PREVIEW);
        let draws = self.draws;
        let id = self.shown.get_mut(&(key.to_owned(), size)).map(|s| {
            s.asked = draws;
            s.id
        });
        if !self.requests.iter().any(|r| r.key == key && r.size == size) {
            self.requests.push(Request {
                key: key.to_owned(),
                size,
                source: source.clone(),
            });
        }
        id
    }

    /// Draws every preview asked for since the last call whose input
    /// changed, loading the images it names, and gives each new one the id
    /// `register` makes for its target (egui's, in the editor). A preview
    /// not asked for in [`KEEP_DRAWS`] calls is let go, its id handed to
    /// `free`. Answers how many it drew.
    pub fn draw(
        &mut self,
        renderer: &mut Renderer,
        textures: &mut Textures,
        meshes: &mut Meshes,
        register: &mut dyn FnMut(&RenderTarget) -> TextureId,
        free: &mut dyn FnMut(TextureId),
    ) -> usize {
        let now = self.draws;
        self.draws += 1;
        self.shown.retain(|_, shown| {
            let kept = shown.asked + KEEP_DRAWS >= now;
            if !kept {
                free(shown.id);
            }
            kept
        });
        if self.requests.is_empty() {
            return 0;
        }
        let mut drawn = 0;
        let device = renderer.context().device().clone();
        let queue = renderer.context().queue().clone();
        let format = renderer.context().format();
        meshes.upload(&device);
        for request in std::mem::take(&mut self.requests) {
            for path in request.source.textures.iter() {
                textures.load(&device, &queue, path);
            }
            let shown = self
                .shown
                .entry((request.key.clone(), request.size))
                .or_insert_with(|| {
                    let target = RenderTarget::new(
                        &device,
                        "surface-preview",
                        format,
                        request.size,
                        request.size,
                        Filter::Linear,
                    );
                    let id = register(&target);
                    Shown {
                        target,
                        id,
                        drawn: None,
                        asked: now,
                    }
                });
            let images = images(textures, &request.source.textures);
            let white = renderer.surface_layouts().white.clone();
            let views = images.views(&white).to_vec();
            let wanted = (request.source.clone(), views, renderer.hdr_3d().format);
            if shown.drawn.as_ref() == Some(&wanted) {
                continue;
            }
            let material = renderer.surface_material(&request.source.standard, &images);
            let Some(sphere) = meshes.gpu(meshes.builtin(BuiltinMesh::Sphere)) else {
                continue;
            };
            let Ok(instance) = Instance3D::new(Affine3A::IDENTITY, [1.0; 4]) else {
                continue;
            };
            let instances = [instance];
            let draws = [MeshDraw3D {
                mesh: sphere,
                material: &material,
                instances: 0..1,
                winding: Winding::CounterClockwise,
            }];
            let scene = Scene3D {
                view: preview_view(),
                instances: &instances,
                draws: &draws,
                clear: wgpu::Color::TRANSPARENT,
                lighting: preview_lighting(),
            };
            renderer.render_3d_into(&shown.target, &scene, &Look3D::default(), Layer::EMPTY);
            shown.drawn = Some(wanted);
            drawn += 1;
        }
        drawn
    }

    /// The target the preview `key` at `size` is drawn into, once drawn.
    #[cfg(test)]
    fn target(&self, key: &str, size: u32) -> Option<&RenderTarget> {
        self.shown.get(&(key.to_owned(), size)).map(|s| &s.target)
    }
}

/// A metre sphere filling most of a square, seen from the front.
fn preview_view() -> View3D {
    View3D::perspective(
        Affine3A::from_rotation_translation(Quat::IDENTITY, Vec3::new(0.0, 0.0, 2.2)),
        30f32.to_radians(),
        1.0,
        0.1,
        None,
    )
}

/// A key light from the upper left in front, and a dim ambient so the
/// shadowed side reads.
fn preview_lighting() -> Lighting3D {
    Lighting3D {
        directional: Some(DirectionalLight {
            direction: Vec3::new(0.6, -0.7, -0.8).normalize(),
            color: [1.0; 3],
            energy: 2.5,
        }),
        ambient: [0.25; 3],
    }
}

#[cfg(test)]
mod tests;
