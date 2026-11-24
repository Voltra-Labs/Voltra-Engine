//! Pictures of a headless run: the world as the player would draw it,
//! read back to the CPU.

use std::fmt;
use std::path::Path;

use voltra_assets::Textures;
use voltra_render::{Filter, RenderTarget, Renderer};

use super::Headless;
use crate::app::draw::shots;
use crate::app::draw::world::WorldDraw;
use crate::app::ui_frame::resolve_world_textures;

/// A drawn frame: tightly packed RGBA bytes, top row first, as a screen
/// shows them (sRGB).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screenshot {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixels, four bytes each (red, green, blue, alpha), row by row, `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

impl Screenshot {
    /// The pixel at `x`, `y` from the top-left, or `None` off the image.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = ((y * self.width + x) * 4) as usize;
        self.rgba.get(i..i + 4).map(|p| [p[0], p[1], p[2], p[3]])
    }

    /// Writes it as a PNG at `path`.
    pub fn save_png(&self, path: &Path) -> Result<(), ScreenshotError> {
        image::save_buffer(
            path,
            &self.rgba,
            self.width,
            self.height,
            image::ColorType::Rgba8,
        )
        .map_err(|e| ScreenshotError::Write(e.to_string()))
    }

    /// The PNG's bytes, for a caller that sends rather than saves it.
    pub fn png_bytes(&self) -> Result<Vec<u8>, ScreenshotError> {
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::write_buffer_with_format(
            &mut bytes,
            &self.rgba,
            self.width,
            self.height,
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        )
        .map_err(|e| ScreenshotError::Write(e.to_string()))?;
        Ok(bytes.into_inner())
    }
}

/// Why a screenshot could not be taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScreenshotError {
    /// No usable GPU adapter on this machine.
    NoDevice,
    /// The PNG would not encode or write.
    Write(String),
}

impl fmt::Display for ScreenshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoDevice => write!(f, "no GPU adapter to draw with"),
            Self::Write(e) => write!(f, "could not write the screenshot: {e}"),
        }
    }
}

impl std::error::Error for ScreenshotError {}

impl Headless {
    /// Draws the world, `width` by `height` pixels, through the scene's
    /// active camera — lit, graded, HUD and all — and reads it back.
    ///
    /// The first call makes a device (a software one in CI) and uploads the
    /// world's textures; later calls reuse them, resizing the targets if the
    /// size changed. [`ScreenshotError::NoDevice`] where there is no adapter
    /// at all.
    pub fn screenshot(&mut self, width: u32, height: u32) -> Result<Screenshot, ScreenshotError> {
        let (width, height) = (width.max(1), height.max(1));
        self.ensure_renderer(width, height)?;
        self.app.resolve_materials();
        let app = &mut self.app;
        let (
            Some(renderer),
            Some(textures),
            Some(atlases),
            Some(fonts),
            Some(target),
            Some(graded),
        ) = (
            app.renderer.as_mut(),
            app.textures.as_mut(),
            app.atlases.as_ref(),
            app.fonts.as_mut(),
            app.scene_target.as_mut(),
            app.post_target.as_mut(),
        )
        else {
            return Err(ScreenshotError::NoDevice);
        };
        let device = renderer.context().device().clone();
        let queue = renderer.context().queue().clone();
        target.resize(&device, width, height);
        graded.resize(&device, width, height);
        renderer.camera.aspect = width as f32 / height as f32;
        // The scene's own camera frames the shot, as in the player.
        app.framing.apply(&app.world, &mut renderer.camera);
        // A texture a tick named since the last shot.
        resolve_world_textures(&mut app.world, textures, &device, &queue);
        voltra_scene::camera::target::resolve_world(&mut app.world, textures, &device, &queue);
        let frame_shots = voltra_scene::camera::shots(&app.world);
        if shots::needs_shots(&frame_shots) {
            voltra_scene::text::prepare(&app.world, fonts, 1.0);
            fonts.upload(textures, &device, &queue);
            let rects = shots::ShotDraw {
                world: &app.world,
                renderer,
                textures,
                atlases,
                fonts,
                silhouettes: app.silhouettes.as_mut(),
                sorting_layers: &app.sorting_layers,
                batch: &mut app.frame_batch,
                tile_meshes: &mut app.tile_meshes,
                gpu_rings: &app.gpu_rings,
                materials: app.materials.as_ref(),
                material_bank: &mut app.material_bank,
                pool: &mut app.camera_pool,
                elapsed: app.elapsed,
            }
            .draw(&frame_shots, (width, height));
            let hud = shots::hud(&app.world, atlases, textures, fonts, (width, height), 1.0);
            let hud_mesh = hud.upload(&device);
            let white = renderer.white_bind_group().clone();
            let hud_draws: Vec<voltra_render::MeshDraw<'_>> = hud
                .ranges
                .iter()
                .map(|range| voltra_render::MeshDraw {
                    texture: match range.texture {
                        Some(handle) => textures.bind_group(handle),
                        None => &white,
                    },
                    normal: None,
                    light_layers: voltra_render::DEFAULT_LIGHT_LAYERS,
                    indices: range.indices.clone(),
                    material: None,
                })
                .collect();
            let placed = shots::placed(&app.camera_pool, &rects);
            renderer.composite(
                graded,
                &placed,
                voltra_render::Layer {
                    mesh: hud_mesh.as_ref(),
                    draws: &hud_draws,
                },
            );
            let rgba = voltra_render::read_rgba8(&device, &queue, graded.texture(), width, height);
            return Ok(Screenshot {
                width,
                height,
                rgba,
            });
        }
        WorldDraw {
            world: &app.world,
            renderer,
            textures,
            atlases,
            fonts,
            silhouettes: app.silhouettes.as_mut(),
            sorting_layers: &app.sorting_layers,
            batch: &mut app.frame_batch,
            tile_meshes: &mut app.tile_meshes,
            gpu_rings: &app.gpu_rings,
            materials: app.materials.as_ref(),
            material_bank: &mut app.material_bank,
            elapsed: app.elapsed,
            scale: 1.0,
        }
        .into_targets(target, graded);
        let rgba = voltra_render::read_rgba8(&device, &queue, graded.texture(), width, height);
        Ok(Screenshot {
            width,
            height,
            rgba,
        })
    }

    /// Makes the device, the texture store and the targets the first time.
    fn ensure_renderer(&mut self, width: u32, height: u32) -> Result<(), ScreenshotError> {
        let app = &mut self.app;
        if app.renderer.is_some() {
            return Ok(());
        }
        let renderer = Renderer::headless(width, height).ok_or(ScreenshotError::NoDevice)?;
        let root = app
            .asset_root
            .clone()
            .unwrap_or_else(voltra_assets::default_root);
        let device = renderer.context().device();
        let queue = renderer.context().queue();
        let format = renderer.context().format();
        let mut textures = Textures::new(device, queue, renderer.texture_layout(), root);
        textures.set_filter(app.texture_filter);
        textures.set_target_format(format);
        resolve_world_textures(&mut app.world, &mut textures, device, queue);
        app.scene_target = Some(RenderTarget::new(
            device,
            "headless-scene",
            format,
            width,
            height,
            Filter::Linear,
        ));
        app.post_target = Some(RenderTarget::new(
            device,
            "headless-graded",
            format,
            width,
            height,
            Filter::Linear,
        ));
        app.textures = Some(textures);
        app.renderer = Some(renderer);
        Ok(())
    }
}
