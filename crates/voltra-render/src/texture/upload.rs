//! Putting pixels on the GPU: the upload, and whether an sRGB texture can
//! be viewed as its plain counterpart.

/// Whether `device` can view an sRGB texture as its plain counterpart
/// (`DownlevelFlags::VIEW_FORMATS`). wgpu 30's GL backend — WebGL2 among it
/// — never can; every other backend always can. Read off the backend
/// because a `Device` does not report its downlevel flags.
pub fn reinterprets_srgb(device: &wgpu::Device) -> bool {
    device.adapter_info().backend != wgpu::Backend::Gl
}

/// Makes a sampled 2D texture of `format` holding `rgba`; with `view_raw`,
/// one that can also be viewed as `format` without its sRGB suffix.
pub(super) fn upload(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    format: wgpu::TextureFormat,
    view_raw: bool,
    rgba: &[u8],
    size: wgpu::Extent3d,
) -> wgpu::Texture {
    let raw = [format.remove_srgb_suffix()];
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        // Reinterpreting these pixels as plain `Rgba8Unorm` has to be
        // declared up front; a view format missing from this list is
        // rejected when the view is created, not when it is sampled.
        view_formats: if view_raw { &raw } else { &[] },
    });
    // `write_texture` has no 256-byte row alignment requirement, unlike
    // `copy_buffer_to_texture`.
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * size.width),
            rows_per_image: Some(size.height),
        },
        size,
    );
    texture
}
