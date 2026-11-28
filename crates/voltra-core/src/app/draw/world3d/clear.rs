//! What a 3D shot's picture is for, and what its target is cleared to and
//! drawn behind.

use voltra_render::render3d::SkyGradient;
use voltra_render::wgpu;
use voltra_scene::{Background3D, CameraClear};

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
/// is, and the sky drawn where nothing else is: the camera's own colour,
/// no sky; else, for an overlay, transparent, so what drew before shows
/// around its world — no sky either, which would hide it; else the
/// environment's `background` — a colour cleared to, a gradient drawn
/// over the renderer's background, or that background itself.
pub(super) fn clear(
    clear: CameraClear,
    overlay: bool,
    background: &Background3D,
    renderer: wgpu::Color,
) -> (wgpu::Color, Option<SkyGradient>) {
    match clear {
        CameraClear::Color([r, g, b, a]) => {
            let a = a.clamp(0.0, 1.0);
            let color = wgpu::Color {
                r: f64::from(r * a),
                g: f64::from(g * a),
                b: f64::from(b * a),
                a: f64::from(a),
            };
            (color, None)
        }
        CameraClear::Background if overlay => (wgpu::Color::TRANSPARENT, None),
        CameraClear::Background => match *background {
            Background3D::Clear => (renderer, None),
            Background3D::Color(c) => {
                let c = c.map(|c| {
                    if c.is_finite() {
                        f64::from(c.max(0.0))
                    } else {
                        0.0
                    }
                });
                let color = wgpu::Color {
                    r: c[0],
                    g: c[1],
                    b: c[2],
                    a: 1.0,
                };
                (color, None)
            }
            Background3D::Gradient { .. } => (renderer, background.sky()),
        },
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
        let none = Background3D::Clear;
        assert_eq!(
            clear(CameraClear::Background, true, &none, background),
            (wgpu::Color::TRANSPARENT, None)
        );
        assert_eq!(
            clear(CameraClear::Background, false, &none, background),
            (background, None)
        );
        let (half, sky) = clear(
            CameraClear::Color([1.0, 0.5, 0.0, 0.5]),
            true,
            &Background3D::DAYLIGHT,
            background,
        );
        assert_eq!((half.r, half.g, half.b, half.a), (0.5, 0.25, 0.0, 0.5));
        assert_eq!(sky, None, "the camera's own colour: no sky");
    }

    #[test]
    fn the_environment_s_background_clears_or_draws_a_sky() {
        let background = wgpu::Color::BLACK;
        let (color, sky) = clear(
            CameraClear::Background,
            false,
            &Background3D::Color([0.2, f32::NAN, 2.0]),
            background,
        );
        assert_eq!(
            (color.r, color.g, color.b, color.a),
            (f64::from(0.2f32), 0.0, 2.0, 1.0)
        );
        assert_eq!(sky, None);
        let (color, sky) = clear(
            CameraClear::Background,
            false,
            &Background3D::DAYLIGHT,
            background,
        );
        assert_eq!(color, background);
        assert_eq!(sky, Background3D::DAYLIGHT.sky());
        assert_eq!(
            clear(
                CameraClear::Background,
                true,
                &Background3D::DAYLIGHT,
                background
            )
            .1,
            None,
            "an overlay hides nothing behind it"
        );
    }
}
