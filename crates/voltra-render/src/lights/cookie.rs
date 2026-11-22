//! A light's cookie: an image laid over its disc that tints and shapes what
//! it gives — Godot's `PointLight2D` texture, Unity's light cookie. A window's
//! panes thrown on the floor, a flashlight's ring, a lamp behind leaves.
//!
//! The image covers the square the light's radius reaches, its right and up
//! along `x` and `y` (the light's own axes in the world: turned, and
//! mirrored, with its entity). Each texel multiplies the light by its colour
//! times its alpha: white leaves it, black takes it away, a colour tints it.

use glam::{Mat2, Vec2};

/// Which cookie a light wears and which way it is laid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cookie {
    /// Into [`Lighting::cookies`](super::Lighting::cookies). One past the
    /// end is no cookie: the light shines as if it had none.
    pub index: u32,
    /// The image's right, in the world. Its length does not matter.
    pub x: Vec2,
    /// The image's up, in the world. Its length does not matter.
    pub y: Vec2,
}

impl Cookie {
    /// The matrix taking an offset from the light (in radii) to where on the
    /// image it falls (`-1..1` on each axis, up positive), as two rows for
    /// the shader; `None` for axes that are not numbers, have no length or
    /// lie along one line.
    pub(super) fn to_image(self) -> Option<[f32; 4]> {
        let (x, y) = (self.x.try_normalize()?, self.y.try_normalize()?);
        let axes = Mat2::from_cols(x, y);
        let det = axes.determinant();
        if !det.is_finite() || det.abs() < 1e-4 {
            return None;
        }
        let inverse = axes.inverse();
        // Rows of the inverse: `image = inverse · offset`.
        Some([
            inverse.x_axis.x,
            inverse.y_axis.x,
            inverse.x_axis.y,
            inverse.y_axis.y,
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(m: [f32; 4], v: Vec2) -> Vec2 {
        Vec2::new(m[0] * v.x + m[1] * v.y, m[2] * v.x + m[3] * v.y)
    }

    #[test]
    fn an_upright_cookie_is_the_identity() {
        let m = Cookie {
            index: 0,
            x: Vec2::X,
            y: Vec2::Y,
        }
        .to_image()
        .expect("lays");
        assert_eq!(apply(m, Vec2::new(0.3, -0.7)), Vec2::new(0.3, -0.7));
    }

    #[test]
    fn a_turned_cookie_turns_back_and_a_mirrored_one_flips() {
        // Turned a quarter: the image's right points up in the world, so a
        // point above the light is on the image's right.
        let turned = Cookie {
            index: 0,
            x: Vec2::Y * 3.0,
            y: Vec2::NEG_X,
        }
        .to_image()
        .expect("lays");
        assert!((apply(turned, Vec2::Y) - Vec2::X).length() < 1e-6);
        let mirrored = Cookie {
            index: 0,
            x: Vec2::NEG_X,
            y: Vec2::Y,
        }
        .to_image()
        .expect("lays");
        assert!((apply(mirrored, Vec2::X) - Vec2::NEG_X).length() < 1e-6);
    }

    #[test]
    fn axes_on_one_line_or_of_no_length_lay_nothing() {
        for (x, y) in [
            (Vec2::X, Vec2::X * 2.0),
            (Vec2::ZERO, Vec2::Y),
            (Vec2::NAN, Vec2::Y),
        ] {
            assert_eq!(Cookie { index: 0, x, y }.to_image(), None, "{x} {y}");
        }
    }
}
