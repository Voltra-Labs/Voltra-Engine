//! Draws egui's tessellated output.
//!
//! `egui-wgpu` exists and does this, but it is pinned to wgpu 29 and would drag
//! a second, incompatible copy of wgpu into the build — its `Device` would be a
//! different type from ours. So the backend is ours.
//!
//! Only `epaint` is needed here, never `egui` itself: this module deals in
//! triangles and texture deltas, and knows nothing about widgets or layout.
//! That keeps the UI's shape a concern of the crate that builds it.

use std::collections::HashMap;
use std::ops::Range;

use bytemuck::{Pod, Zeroable};
use epaint::textures::{TextureFilter, TextureOptions, TextureWrapMode, TexturesDelta};
use epaint::{ClippedPrimitive, ImageData, ImageDelta, Primitive, TextureId};

use crate::{shader, texture};

/// Where egui is being drawn, in physical pixels plus the scale that maps
/// egui's logical points onto them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenDescriptor {
    pub width: u32,
    pub height: u32,
    /// Physical pixels per logical point — the display's scale factor.
    pub pixels_per_point: f32,
}

impl ScreenDescriptor {
    /// The size egui lays out against, which is the physical size divided by
    /// the scale factor.
    pub fn size_in_points(&self) -> [f32; 2] {
        [
            self.width as f32 / self.pixels_per_point,
            self.height as f32 / self.pixels_per_point,
        ]
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Locals {
    screen_size: [f32; 2],
    /// The shader-side struct is padded to 16 bytes; this keeps the Rust side
    /// the same size so the write is not short.
    _padding: [f32; 2],
}

/// One texture egui asked us to hold, with the bind group that draws it.
struct BoundTexture {
    /// `None` for a view registered by the application: the bind group borrows
    /// it, but the texture is not ours to keep alive or to write into.
    texture: Option<wgpu::Texture>,
    bind_group: wgpu::BindGroup,
    /// Kept so a delta that changes only the filtering can be detected.
    options: Option<TextureOptions>,
}

/// One `draw_indexed` call, resolved during `prepare`.
struct DrawCommand {
    texture: TextureId,
    indices: Range<u32>,
    /// Added to every index, so each mesh's indices stay zero-based.
    base_vertex: i32,
    scissor: [u32; 4],
}

/// Renders egui. Feed it `prepare`, then `render` inside a pass.
pub struct EguiBackend {
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    texture_layout: wgpu::BindGroupLayout,

    textures: HashMap<TextureId, BoundTexture>,
    samplers: HashMap<TextureOptions, wgpu::Sampler>,
    /// Freed one frame late — see [`EguiBackend::prepare`].
    pending_free: Vec<TextureId>,
    next_user_id: u64,

    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    draws: Vec<DrawCommand>,
}

impl EguiBackend {
    /// `format` must match the attachment this will draw into.
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let module = shader::create_module(device, "egui", shader::EGUI);

        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("egui-locals"),
            size: size_of::<Locals>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("egui-locals-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("egui-locals-bind-group"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });

        // Deliberately the same layout the sprite pipeline uses: a texture at 0
        // and its sampler at 1. That is what lets a `RenderTarget`'s bind group
        // be handed straight to egui as the viewport image.
        let texture_layout = texture::bind_group_layout(device);

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("egui-layout"),
            bind_group_layouts: &[Some(&uniform_layout), Some(&texture_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("egui-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: size_of::<epaint::Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    // pos, uv, then the colour as four packed sRGB bytes.
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Uint32],
                })],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                // egui emits both windings, so culling would eat half the UI.
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some(if format.is_srgb() {
                    "fs_main_srgb_target"
                } else {
                    "fs_main_unorm_target"
                }),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    // Not `ALPHA_BLENDING`: egui's colours arrive with alpha
                    // already multiplied in, so the source must not be scaled
                    // by it a second time. The alpha channel accumulates
                    // coverage instead, which matters when egui draws into a
                    // transparent target.
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::OneMinusDstAlpha,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline,
            uniform,
            uniform_bind_group,
            texture_layout,
            textures: HashMap::new(),
            samplers: HashMap::new(),
            pending_free: Vec::new(),
            next_user_id: 0,
            vertices: empty_buffer(device, "egui-vertices", wgpu::BufferUsages::VERTEX),
            indices: empty_buffer(device, "egui-indices", wgpu::BufferUsages::INDEX),
            draws: Vec::new(),
        }
    }

    /// Uploads this frame's textures and geometry. Call before [`Self::render`].
    ///
    /// `delta.free` is not applied now but on the *next* call. egui's contract
    /// is that a freed texture is dead only once the frame that reported it has
    /// been drawn, and `primitives` here may still reference one.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        primitives: &[ClippedPrimitive],
        delta: &TexturesDelta,
        screen: ScreenDescriptor,
    ) {
        for id in self.pending_free.drain(..) {
            self.textures.remove(&id);
        }
        self.pending_free.extend_from_slice(&delta.free);

        for (id, image_delta) in &delta.set {
            self.set_texture(device, queue, *id, image_delta);
        }

        queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&Locals {
                screen_size: screen.size_in_points(),
                _padding: [0.0; 2],
            }),
        );

        self.draws.clear();
        let mut vertices: Vec<epaint::Vertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        for ClippedPrimitive {
            clip_rect,
            primitive,
        } in primitives
        {
            let Primitive::Mesh(mesh) = primitive else {
                // Paint callbacks let a widget issue its own draw calls. The
                // editor shows the viewport as a registered texture instead, so
                // there is nothing here to honour yet.
                log::warn!("egui paint callbacks are not supported; primitive skipped");
                continue;
            };

            let scissor = scissor_rect(clip_rect, screen);
            // A fully clipped mesh still costs a draw call, and a zero-sized
            // scissor is a validation error on some backends.
            if scissor[2] == 0 || scissor[3] == 0 || mesh.indices.is_empty() {
                continue;
            }

            let start = indices.len() as u32;
            indices.extend_from_slice(&mesh.indices);
            self.draws.push(DrawCommand {
                texture: mesh.texture_id,
                indices: start..indices.len() as u32,
                base_vertex: vertices.len() as i32,
                scissor,
            });
            vertices.extend_from_slice(&mesh.vertices);
        }

        write_growable(
            device,
            queue,
            &mut self.vertices,
            "egui-vertices",
            wgpu::BufferUsages::VERTEX,
            bytemuck::cast_slice(&vertices),
        );
        write_growable(
            device,
            queue,
            &mut self.indices,
            "egui-indices",
            wgpu::BufferUsages::INDEX,
            bytemuck::cast_slice(&indices),
        );
    }

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

    /// Hands egui a texture the application owns, such as the viewport target.
    ///
    /// The returned id stays valid until [`Self::free_view`]; after a resize
    /// call [`Self::update_view`], because the old view points at freed memory.
    pub fn register_view(
        &mut self,
        device: &wgpu::Device,
        view: &wgpu::TextureView,
        filter: crate::Filter,
    ) -> TextureId {
        let id = TextureId::User(self.next_user_id);
        self.next_user_id += 1;
        self.set_view(device, id, view, filter);
        id
    }

    /// Points an id registered by [`Self::register_view`] at a different view.
    pub fn update_view(
        &mut self,
        device: &wgpu::Device,
        id: TextureId,
        view: &wgpu::TextureView,
        filter: crate::Filter,
    ) {
        self.set_view(device, id, view, filter);
    }

    pub fn free_view(&mut self, id: TextureId) {
        self.textures.remove(&id);
    }

    fn set_view(
        &mut self,
        device: &wgpu::Device,
        id: TextureId,
        view: &wgpu::TextureView,
        filter: crate::Filter,
    ) {
        let options = TextureOptions {
            magnification: filter_of(filter),
            minification: filter_of(filter),
            ..Default::default()
        };
        let bind_group = self.bind(device, view, options);
        self.textures.insert(
            id,
            BoundTexture {
                texture: None,
                bind_group,
                options: None,
            },
        );
    }

    fn set_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        id: TextureId,
        delta: &ImageDelta,
    ) {
        let ImageData::Color(image) = &delta.image;
        let width = image.width() as u32;
        let height = image.height() as u32;
        let pixels: &[u8] = bytemuck::cast_slice(&image.pixels);

        // `pos` means this is a patch of an existing texture — egui grows its
        // font atlas by re-uploading only the new glyphs.
        let existing = delta.pos.and_then(|_| self.textures.remove(&id));

        let (texture, origin, reuse) = match (existing, delta.pos) {
            (Some(bound), Some(pos)) => {
                let Some(texture) = bound.texture else {
                    log::warn!("egui tried to patch {id:?}, which the application owns");
                    return;
                };
                let origin = wgpu::Origin3d {
                    x: pos[0] as u32,
                    y: pos[1] as u32,
                    z: 0,
                };
                // Only the filtering decides the bind group, so an unchanged
                // patch keeps the one it already had.
                let reuse = (bound.options == Some(delta.options)).then_some(bound.bind_group);
                (texture, origin, reuse)
            }
            _ => {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("egui-texture"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    // Unorm, not UnormSrgb: egui's pixels are already gamma
                    // encoded and its blending happens in that space, so the
                    // shader converts at the end rather than the sampler at the
                    // start.
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                (texture, wgpu::Origin3d::ZERO, None)
            }
        };

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let bind_group = reuse.unwrap_or_else(|| {
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.bind(device, &view, delta.options)
        });

        self.textures.insert(
            id,
            BoundTexture {
                texture: Some(texture),
                bind_group,
                options: Some(delta.options),
            },
        );
    }

    fn bind(
        &mut self,
        device: &wgpu::Device,
        view: &wgpu::TextureView,
        options: TextureOptions,
    ) -> wgpu::BindGroup {
        // egui uses a handful of option sets at most, and a sampler per texture
        // would mean a new one for every glyph atlas patch.
        let sampler = self
            .samplers
            .entry(options)
            .or_insert_with(|| create_sampler(device, options));

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("egui-texture-bind-group"),
            layout: &self.texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }
}

fn filter_of(filter: crate::Filter) -> TextureFilter {
    match filter {
        crate::Filter::Linear => TextureFilter::Linear,
        crate::Filter::Nearest => TextureFilter::Nearest,
    }
}

fn create_sampler(device: &wgpu::Device, options: TextureOptions) -> wgpu::Sampler {
    let address_mode = match options.wrap_mode {
        TextureWrapMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
        TextureWrapMode::Repeat => wgpu::AddressMode::Repeat,
        TextureWrapMode::MirroredRepeat => wgpu::AddressMode::MirrorRepeat,
    };
    let mode = |filter| match filter {
        TextureFilter::Nearest => wgpu::FilterMode::Nearest,
        TextureFilter::Linear => wgpu::FilterMode::Linear,
    };

    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("egui-sampler"),
        address_mode_u: address_mode,
        address_mode_v: address_mode,
        address_mode_w: address_mode,
        mag_filter: mode(options.magnification),
        min_filter: mode(options.minification),
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    })
}

/// Converts an egui clip rect from logical points into attachment pixels.
///
/// Returns `[x, y, width, height]`. Clamping is not defensive tidiness: egui
/// happily emits rects reaching past the screen, and a scissor outside the
/// attachment is a validation error that kills the frame.
fn scissor_rect(clip: &epaint::Rect, screen: ScreenDescriptor) -> [u32; 4] {
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

fn empty_buffer(device: &wgpu::Device, label: &str, usage: wgpu::BufferUsages) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: 0,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Writes `data`, reallocating only when it no longer fits.
///
/// The UI's triangle count changes every frame as panels open and text scrolls,
/// so a fixed buffer would either overflow or be sized for the worst case.
fn write_growable(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buffer: &mut wgpu::Buffer,
    label: &str,
    usage: wgpu::BufferUsages,
    data: &[u8],
) {
    if data.is_empty() {
        return;
    }

    if (data.len() as wgpu::BufferAddress) > buffer.size() {
        // Doubling keeps a steadily growing UI from reallocating every frame.
        let size = (data.len() as wgpu::BufferAddress).next_power_of_two();
        *buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
    }

    queue.write_buffer(buffer, 0, data);
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
    fn points_convert_to_pixels_by_the_scale_factor() {
        assert_eq!(SCREEN.size_in_points(), [400.0, 300.0]);
    }

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

    #[test]
    fn locals_match_the_padded_shader_struct() {
        assert_eq!(size_of::<Locals>(), 16);
    }

    #[test]
    fn vertex_layout_matches_epaint() {
        // pos and uv are two f32s each, colour is four packed bytes.
        assert_eq!(size_of::<epaint::Vertex>(), 20);
    }
}
