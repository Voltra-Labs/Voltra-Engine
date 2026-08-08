//! Turns the meshes `EguiBackend::prepare` built into draw calls.
//!
//! Each clipped mesh becomes one `DrawCommand`, scissored to its clip rect;
//! `EguiBackend::render` walks the list and issues the indexed draws.

use std::ops::Range;

use epaint::TextureId;

use super::{EguiBackend, ScreenDescriptor};

/// One `draw_indexed` call, resolved during `prepare`.
pub(super) struct DrawCommand {
    pub(super) texture: TextureId,
    pub(super) indices: Range<u32>,
    /// Added to every index, so each mesh's indices stay zero-based.
    pub(super) base_vertex: i32,
    pub(super) scissor: [u32; 4],
}

impl EguiBackend {
    /// Records the draw calls built by the last [`Self::prepare`].
    ///
    /// The pass must target an attachment of the format this backend was built
    /// with, and cover the whole `ScreenDescriptor` — the scissor rects are in
    /// attachment pixels.
    pub fn render(&self, pass: &mut wgpu::RenderPass<'_>) {
        if self.draws.is_empty() {
            return;
        }

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.uniform_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);

        for draw in &self.draws {
            let Some(bound) = self.textures.get(&draw.texture) else {
                log::warn!("egui asked for missing texture {:?}", draw.texture);
                continue;
            };
            pass.set_bind_group(1, &bound.bind_group, &[]);
            let [x, y, width, height] = draw.scissor;
            pass.set_scissor_rect(x, y, width, height);
            pass.draw_indexed(draw.indices.clone(), draw.base_vertex, 0..1);
        }
    }
}

/// Converts an egui clip rect from logical points into attachment pixels.
///
/// Returns `[x, y, width, height]`. Clamping is not defensive tidiness: egui
/// happily emits rects reaching past the screen, and a scissor outside the
/// attachment is a validation error that kills the frame.
pub(super) fn scissor_rect(clip: &epaint::Rect, screen: ScreenDescriptor) -> [u32; 4] {
    let ppp = screen.pixels_per_point;
    let min_x = (clip.min.x * ppp).round().clamp(0.0, screen.width as f32) as u32;
    let min_y = (clip.min.y * ppp).round().clamp(0.0, screen.height as f32) as u32;
    let max_x = (clip.max.x * ppp).round().clamp(0.0, screen.width as f32) as u32;
    let max_y = (clip.max.y * ppp).round().clamp(0.0, screen.height as f32) as u32;

    [
        min_x,
        min_y,
        max_x.saturating_sub(min_x),
        max_y.saturating_sub(min_y),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use epaint::{pos2, Rect};

    const SCREEN: ScreenDescriptor = ScreenDescriptor {
        width: 800,
        height: 600,
        pixels_per_point: 2.0,
    };

    #[test]
    fn scissor_scales_a_clip_rect_into_pixels() {
        let clip = Rect::from_min_max(pos2(10.0, 20.0), pos2(110.0, 70.0));
        assert_eq!(scissor_rect(&clip, SCREEN), [20, 40, 200, 100]);
    }

    #[test]
    fn scissor_is_clamped_to_the_attachment() {
        // egui emits rects well past the screen edge; passing one through is a
        // validation error, not a clipped triangle.
        let clip = Rect::from_min_max(pos2(-50.0, -50.0), pos2(9999.0, 9999.0));
        assert_eq!(scissor_rect(&clip, SCREEN), [0, 0, 800, 600]);
    }

    #[test]
    fn a_clip_rect_entirely_offscreen_has_no_area() {
        let clip = Rect::from_min_max(pos2(1000.0, 1000.0), pos2(2000.0, 2000.0));
        let [_, _, width, height] = scissor_rect(&clip, SCREEN);
        assert_eq!((width, height), (0, 0));
    }

    #[test]
    fn an_inverted_clip_rect_does_not_underflow() {
        // Nothing forbids max < min, and `max - min` on u32 would panic.
        let clip = Rect::from_min_max(pos2(200.0, 200.0), pos2(100.0, 100.0));
        let [_, _, width, height] = scissor_rect(&clip, SCREEN);
        assert_eq!((width, height), (0, 0));
    }
}
