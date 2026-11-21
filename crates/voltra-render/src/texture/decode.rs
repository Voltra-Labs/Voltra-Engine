//! Decoding a PNG, apart from uploading it: no device needed, so any
//! thread can do it.

use super::TextureError;

/// A PNG's pixels, decoded and not yet uploaded: what can be made on any
/// thread, since it needs no device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedImage {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Decodes a PNG to RGBA8. Pure CPU work — the inflate is most of a
/// texture's load — so callers may run it off the thread that uploads.
pub fn decode_png(bytes: &[u8]) -> Result<DecodedImage, TextureError> {
    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map_err(TextureError::Decode)?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    Ok(DecodedImage {
        rgba: rgba.into_raw(),
        width,
        height,
    })
}
