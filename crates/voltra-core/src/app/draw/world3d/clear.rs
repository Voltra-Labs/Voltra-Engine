//! What a 3D shot's picture is for, and what its target is cleared to.

use voltra_render::wgpu;
use voltra_scene::CameraClear;

/// What a 3D shot's picture is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum Picture3D {
    /// A screen shot drawn over nothing of the other dimension.
    Opaque,
    /// A screen shot over another dimension's picture: transparent where
    /// it draws nothing.
    Overlay,
    /// A render texture, written in straight alpha for the sprites that
    /// show it.
    Texture,
}

/// What a 3D shot's target is cleared to, premultiplied as the picture
/// is: the camera's own colour; else transparent for an overlay, so what
/// drew before shows around its world; else the renderer's background.
pub(super) fn clear(clear: CameraClear, overlay: bool, background: wgpu::Color) -> wgpu::Color {
    match clear {
        CameraClear::Color([r, g, b, a]) => {
            let a = a.clamp(0.0, 1.0);
            wgpu::Color {
                r: f64::from(r * a),
                g: f64::from(g * a),
                b: f64::from(b * a),
                a: f64::from(a),
            }
        }
        CameraClear::Background if overlay => wgpu::Color::TRANSPARENT,
        CameraClear::Background => background,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_overlay_clears_transparent_and_a_colour_of_its_own_stays() {
        let background = wgpu::Color {
            r: 0.1,
            g: 0.1,
            b: 0.12,
            a: 1.0,
        };
        assert_eq!(
            clear(CameraClear::Background, true, background),
            wgpu::Color::TRANSPARENT
        );
        assert_eq!(
            clear(CameraClear::Background, false, background),
            background
        );
        let half = clear(CameraClear::Color([1.0, 0.5, 0.0, 0.5]), true, background);
        assert_eq!((half.r, half.g, half.b, half.a), (0.5, 0.25, 0.0, 0.5));
    }
}
