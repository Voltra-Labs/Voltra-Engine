//! The pieces a frame is drawn from: text made ready, the HUD's batch, the
//! draw list, the lighting, what the camera sees.

use voltra_assets::{Fonts, Textures};
use voltra_ecs::World;
use voltra_render::{wgpu, MeshDraw};
use voltra_scene::sprite::sheets::Sheets;
use voltra_scene::{ui, SpriteBatch, UiLayout};

/// Rasterizes the glyphs the world's texts draw and uploads the atlases
/// that changed — before the batch, so its UVs are taken against the atlas
/// as it will be sampled.
pub(super) fn prepare_text(
    world: &World,
    fonts: &mut Fonts,
    textures: &mut Textures,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    ui_scale: f32,
) {
    voltra_scene::text::prepare(world, fonts, ui_scale);
    fonts.upload(textures, device, queue);
}

/// Physical pixels per point: the window's scale factor, or 1 before there
/// is a window to ask.
pub(super) fn ui_scale(window: Option<&winit::window::Window>) -> f32 {
    window.map_or(1.0, |window| window.scale_factor() as f32)
}

/// The screen-space batch for a target `width` × `height` physical pixels.
///
/// Laid out in points — the size divided by `scale` — so a HUD authored at
/// 16 points in from the corner is 16 points in on any display.
pub(super) fn ui_batch(
    world: &World,
    sheets: Sheets<'_>,
    width: u32,
    height: u32,
    scale: f32,
) -> SpriteBatch {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let screen = voltra_render::glam::Vec2::new(width as f32, height as f32) / scale;
    let layout = UiLayout::from_world(world, screen);
    ui::mesh::batch(world, sheets, &layout, scale)
}

/// Turns a batch's texture-run ranges into the renderer's draw list.
///
/// `white` stands in for `None`: an untextured range still needs a bind
/// group, and the renderer's own white one is what every flat-coloured
/// sprite drew against before textures existed. Both `textures` and `white`
/// share `'a` so the returned draws can borrow either.
pub(super) fn mesh_draws<'a>(
    batch: &SpriteBatch,
    textures: &'a Textures,
    white: &'a wgpu::BindGroup,
) -> Vec<MeshDraw<'a>> {
    batch
        .ranges
        .iter()
        .map(|range| MeshDraw {
            texture: match range.texture {
                Some(handle) => textures.bind_group(handle),
                None => white,
            },
            normal: None,
            light_layers: voltra_render::DEFAULT_LIGHT_LAYERS,
            primitives: range.primitives.clone(),
            material: None,
        })
        .collect()
}

/// The frame's lighting, with the lights `camera` can see lit. The editor
/// previews it too, as it previews the grade. Sprite-shaped casters' frames
/// are traced first, through `silhouettes`, when there are any.
pub(super) fn lighting(
    world: &World,
    camera: &voltra_render::Camera2D,
    sheets: Sheets<'_>,
    silhouettes: Option<&mut voltra_assets::Silhouettes>,
    textures: &voltra_assets::Textures,
) -> voltra_render::Lighting {
    let silhouettes = silhouettes.map(|silhouettes| {
        voltra_scene::shadow_caster::prepare(world, sheets, silhouettes);
        &*silhouettes
    });
    let sprites = silhouettes.map(|silhouettes| voltra_scene::shadow_caster::SpriteShapes {
        sheets,
        silhouettes,
    });
    voltra_scene::light::gather(
        world,
        view_min(camera),
        view_max(camera),
        sprites,
        Some(textures),
    )
}

/// The lower-left corner of what `camera` sees, in world units.
pub(super) fn view_min(camera: &voltra_render::Camera2D) -> voltra_render::glam::Vec2 {
    camera.position - camera.half_extents()
}

/// The upper-right corner of what `camera` sees, in world units.
pub(super) fn view_max(camera: &voltra_render::Camera2D) -> voltra_render::glam::Vec2 {
    camera.position + camera.half_extents()
}

/// The elapsed clock as the batch takes it. Wrapped at a day, so an `f32`
/// keeps millisecond precision however long the game has been open.
pub(super) fn clock_seconds(elapsed: f64) -> f32 {
    (elapsed % 86_400.0) as f32
}
