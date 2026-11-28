//! A material's uniforms: their values, their WGSL layout, and the buffer
//! one frame's blocks of them go in.

use std::collections::BTreeMap;

/// One uniform's value, and so its type.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Uniform {
    /// `f32`.
    Float(f32),
    /// `vec2<f32>`.
    Vec2((f32, f32)),
    /// `vec4<f32>`: RGBA, or any four numbers.
    Color((f32, f32, f32, f32)),
}

impl Uniform {
    /// Its WGSL type.
    pub fn wgsl(&self) -> &'static str {
        match self {
            Self::Float(_) => "f32",
            Self::Vec2(_) => "vec2<f32>",
            Self::Color(_) => "vec4<f32>",
        }
    }

    /// Its size and alignment in a uniform buffer, in bytes.
    fn size_align(&self) -> (usize, usize) {
        match self {
            Self::Float(_) => (4, 4),
            Self::Vec2(_) => (8, 8),
            Self::Color(_) => (16, 16),
        }
    }

    fn floats(&self) -> Vec<f32> {
        match *self {
            Self::Float(a) => vec![a],
            Self::Vec2((a, b)) => vec![a, b],
            Self::Color((a, b, c, d)) => vec![a, b, c, d],
        }
    }

    /// Whether every number in it is finite.
    pub fn is_finite(&self) -> bool {
        self.floats().iter().all(|f| f.is_finite())
    }

    /// The same kind with every number that is not finite made 0, as a
    /// uniform block writes it.
    pub fn finite(&self) -> Self {
        let f = |v: f32| if v.is_finite() { v } else { 0.0 };
        match *self {
            Self::Float(a) => Self::Float(f(a)),
            Self::Vec2((a, b)) => Self::Vec2((f(a), f(b))),
            Self::Color((a, b, c, d)) => Self::Color((f(a), f(b), f(c), f(d))),
        }
    }

    /// Whether `other` is of the same type.
    pub fn same_kind(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

/// The most bytes one material's uniforms take: one block of the frame's
/// buffer. Sixteen `vec4`s.
pub const BLOCK_SIZE: usize = 256;

/// Where each uniform sits in a material's block: by name, in name order,
/// at the alignment WGSL's uniform address space asks — 4 for `f32`, 8 for
/// `vec2`, 16 for `vec4` — the whole rounded to 16.
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    fields: Vec<(String, Uniform, usize)>,
    size: usize,
}

impl Layout {
    /// The layout of `uniforms`, the defaults' types giving each its type;
    /// `Err` when they do not fit in a [`BLOCK_SIZE`] block or a name is no
    /// WGSL identifier.
    pub fn new(uniforms: &BTreeMap<String, Uniform>) -> Result<Self, String> {
        let mut offset: usize = 0;
        let mut fields = Vec::new();
        for (name, value) in uniforms {
            if !is_identifier(name) {
                return Err(format!("{name:?} is not a name WGSL takes"));
            }
            let (size, align) = value.size_align();
            offset = offset.next_multiple_of(align);
            fields.push((name.clone(), *value, offset));
            offset += size;
        }
        let size = offset.next_multiple_of(16).max(16);
        if size > BLOCK_SIZE {
            return Err(format!(
                "the uniforms take {size} bytes; a material has {BLOCK_SIZE}"
            ));
        }
        Ok(Self { fields, size })
    }

    /// The struct the shader reads them through.
    pub fn wgsl_struct(&self) -> String {
        let mut out = String::from("struct Material {\n");
        if self.fields.is_empty() {
            // WGSL has no empty struct; nothing reads this.
            out.push_str("    unused: vec4<f32>,\n");
        }
        for (name, value, _) in &self.fields {
            out.push_str(&format!("    {name}: {},\n", value.wgsl()));
        }
        out.push_str("}\n");
        out
    }

    /// [`wgsl_struct`](Self::wgsl_struct) with its first member aligned
    /// to 16, so the struct's size is a multiple of 16 — [`size`](Self::size)
    /// exactly. WebGL2 refuses a uniform binding whose type is not
    /// (`DownlevelFlags::BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED`): a lone
    /// `f32` or `vec2` would be 4 or 8 bytes. What a surface material's
    /// block is declared as.
    pub fn wgsl_struct_aligned(&self) -> String {
        self.wgsl_struct().replacen(
            "struct Material {\n    ",
            "struct Material {\n    @align(16) ",
            1,
        )
    }

    /// Bytes of the block itself, before rounding to a [`BLOCK_SIZE`].
    pub fn size(&self) -> usize {
        self.size
    }

    /// The block for these uniforms: each the value in `overrides` when it
    /// has one of the right type, its default otherwise.
    pub fn block(&self, overrides: &BTreeMap<String, Uniform>) -> [u8; BLOCK_SIZE] {
        let mut block = [0u8; BLOCK_SIZE];
        for (name, default, offset) in &self.fields {
            let value = overrides
                .get(name)
                .filter(|v| v.same_kind(default))
                .unwrap_or(default);
            for (i, f) in value.floats().iter().enumerate() {
                let f = if f.is_finite() { *f } else { 0.0 };
                let at = offset + i * 4;
                block[at..at + 4].copy_from_slice(&f.to_le_bytes());
            }
        }
        block
    }
}

/// Whether `name` is an identifier WGSL takes, and not one it reserves for
/// itself (a leading `__`).
pub(crate) fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !name.starts_with("__")
}

/// One frame's uniform blocks, in one buffer, bound with a dynamic offset
/// per draw: a material's draws differ only in where their block starts.
pub struct MaterialUniforms {
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// Blocks it holds room for.
    capacity: usize,
    /// Bytes from one block to the next: the device's offset alignment, at
    /// least a block.
    stride: usize,
}

impl MaterialUniforms {
    /// A uniform buffer on `device` and its bind group against `layout`.
    pub fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout) -> Self {
        let align = device.limits().min_uniform_buffer_offset_alignment as usize;
        let stride = BLOCK_SIZE.next_multiple_of(align.max(1));
        Self::with_capacity(device, layout, stride, 16)
    }

    fn with_capacity(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        stride: usize,
        capacity: usize,
    ) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("material-uniforms"),
            size: (stride * capacity) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("material-uniforms"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(BLOCK_SIZE as u64),
                }),
            }],
        });
        Self {
            buffer,
            bind_group,
            capacity,
            stride,
        }
    }

    /// Uploads this frame's `blocks`, growing the buffer if they do not fit,
    /// and answers each block's dynamic offset, in order.
    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        blocks: &[[u8; BLOCK_SIZE]],
    ) -> Vec<u32> {
        if blocks.len() > self.capacity {
            let capacity = blocks.len().next_power_of_two();
            *self = Self::with_capacity(device, layout, self.stride, capacity);
        }
        let mut bytes = vec![0u8; self.stride * blocks.len()];
        for (i, block) in blocks.iter().enumerate() {
            bytes[i * self.stride..i * self.stride + BLOCK_SIZE].copy_from_slice(block);
        }
        if !bytes.is_empty() {
            queue.write_buffer(&self.buffer, 0, &bytes);
        }
        (0..blocks.len())
            .map(|i| (i * self.stride) as u32)
            .collect()
    }

    /// The bind group over the frame's uniform blocks.
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uniforms(list: &[(&str, Uniform)]) -> BTreeMap<String, Uniform> {
        list.iter().map(|(n, v)| (n.to_string(), *v)).collect()
    }

    #[test]
    fn fields_go_in_name_order_at_their_alignment() {
        let layout = Layout::new(&uniforms(&[
            ("amount", Uniform::Float(0.5)),
            ("color", Uniform::Color((1.0, 0.0, 0.0, 1.0))),
            ("offset", Uniform::Vec2((2.0, 3.0))),
        ]))
        .expect("fits");
        // amount at 0, color at 16 (after padding), offset at 32; size 48.
        assert_eq!(layout.size(), 48);
        let block = layout.block(&BTreeMap::new());
        let f = |at: usize| f32::from_le_bytes(block[at..at + 4].try_into().expect("four bytes"));
        assert_eq!(f(0), 0.5);
        assert_eq!(f(16), 1.0);
        assert_eq!(f(32), 2.0);
        assert_eq!(f(36), 3.0);
        assert!(layout.wgsl_struct().contains("color: vec4<f32>"));
    }

    #[test]
    fn overrides_of_the_right_kind_replace_defaults() {
        let layout = Layout::new(&uniforms(&[("flash", Uniform::Float(0.0))])).expect("fits");
        let block = layout.block(&uniforms(&[("flash", Uniform::Float(1.0))]));
        assert_eq!(
            f32::from_le_bytes(block[0..4].try_into().expect("four")),
            1.0
        );
        let wrong = layout.block(&uniforms(&[("flash", Uniform::Vec2((5.0, 5.0)))]));
        assert_eq!(
            f32::from_le_bytes(wrong[0..4].try_into().expect("four")),
            0.0
        );
        let nan = layout.block(&uniforms(&[("flash", Uniform::Float(f32::NAN))]));
        assert_eq!(f32::from_le_bytes(nan[0..4].try_into().expect("four")), 0.0);
    }

    #[test]
    fn too_many_or_badly_named_uniforms_are_refused() {
        let many: BTreeMap<String, Uniform> = (0..17)
            .map(|i| (format!("c{i:02}"), Uniform::Color((0.0, 0.0, 0.0, 0.0))))
            .collect();
        assert!(Layout::new(&many).is_err());
        assert!(Layout::new(&uniforms(&[("not a name", Uniform::Float(0.0))])).is_err());
        assert!(Layout::new(&uniforms(&[("__x", Uniform::Float(0.0))])).is_err());
        let empty = Layout::new(&BTreeMap::new()).expect("nothing fits");
        assert_eq!(empty.size(), 16);
        assert!(empty.wgsl_struct().contains("unused"));
    }
}
