//! The sprite component: a coloured quad, sized by its entity's transform.

/// A coloured quad. Its size comes from the entity's [`Transform`] scale.
///
/// [`Transform`]: crate::transform::Transform
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sprite {
    /// Multiplied with the bound texture. White leaves the texture as-is.
    pub color: [f32; 4],
}

impl Default for Sprite {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

impl Sprite {
    pub fn new(color: [f32; 4]) -> Self {
        Self { color }
    }
}
