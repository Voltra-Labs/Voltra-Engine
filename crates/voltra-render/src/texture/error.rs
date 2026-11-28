//! Why a texture could not be created.

use std::error::Error;
use std::fmt;

/// Why a texture could not be created.
#[derive(Debug)]
pub enum TextureError {
    /// The bytes were not a PNG, or were a corrupt one.
    Decode(image::ImageError),
    /// The pixel data does not match the stated dimensions.
    SizeMismatch {
        /// The number of bytes expected.
        expected: usize,
        /// The number of bytes given.
        actual: usize,
        /// The stated width, in pixels.
        width: u32,
        /// The stated height, in pixels.
        height: u32,
    },
}

impl fmt::Display for TextureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(e) => write!(f, "failed to decode image: {e}"),
            Self::SizeMismatch {
                expected,
                actual,
                width,
                height,
            } => write!(
                f,
                "pixel data is {actual} bytes but {width}x{height} RGBA needs {expected}"
            ),
        }
    }
}

impl Error for TextureError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Decode(e) => Some(e),
            Self::SizeMismatch { .. } => None,
        }
    }
}
