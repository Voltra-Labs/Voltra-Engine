//! The textures a graph's created slots borrow, kept from frame to frame.

use crate::target::RenderTarget;
use crate::texture::Filter;

/// What a created slot needs: a size and a format. Two slots alike may share
/// a texture when their uses do not overlap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureDesc {
    /// Names the texture in GPU debug tools.
    pub label: &'static str,
    /// Width, in texels.
    pub width: u32,
    /// Height, in texels.
    pub height: u32,
    /// Texel format.
    pub format: wgpu::TextureFormat,
}

impl TextureDesc {
    fn fits(&self, other: &TextureDesc) -> bool {
        (self.width, self.height, self.format) == (other.width, other.height, other.format)
    }
}

struct Entry {
    desc: TextureDesc,
    target: RenderTarget,
    /// Lent to a slot right now.
    busy: bool,
    /// Frames since anything used it.
    idle: u32,
}

/// Transient textures, reused across frames and, once a slot is done with
/// one, within a frame.
///
/// A texture no frame has asked for in [`TexturePool::KEEP_FRAMES`] is
/// dropped: a resized window leaves its old sizes behind, and they would
/// otherwise stay allocated for good.
#[derive(Default)]
pub struct TexturePool {
    entries: Vec<Entry>,
}

impl TexturePool {
    /// How long an unused texture is kept.
    pub const KEEP_FRAMES: u32 = 3;

    /// Textures held, busy or not.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no texture is held.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(super) fn begin_frame(&mut self) {
        for entry in &mut self.entries {
            entry.busy = false;
            entry.idle = entry.idle.saturating_add(1);
        }
    }

    pub(super) fn acquire(&mut self, device: &wgpu::Device, desc: &TextureDesc) -> usize {
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| !entry.busy && entry.desc.fits(desc))
        {
            let entry = &mut self.entries[index];
            entry.busy = true;
            entry.idle = 0;
            return index;
        }
        self.entries.push(Entry {
            desc: *desc,
            target: RenderTarget::new(
                device,
                desc.label,
                desc.format,
                desc.width,
                desc.height,
                Filter::Linear,
            ),
            busy: true,
            idle: 0,
        });
        self.entries.len() - 1
    }

    pub(super) fn view(&self, index: usize) -> &wgpu::TextureView {
        self.entries[index].target.view()
    }

    pub(super) fn release(&mut self, index: usize) {
        self.entries[index].busy = false;
    }

    pub(super) fn end_frame(&mut self) {
        self.entries.retain(|entry| entry.idle < Self::KEEP_FRAMES);
    }
}
