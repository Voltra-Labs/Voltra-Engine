//! 2D lights, as passes of the frame's [`RenderGraph`].
//!
//! A light map the frame's size is cleared to the ambient colour; each
//! point light is added onto it as a quad around the light, through the
//! world camera; the drawn scene is then multiplied by it, pixel for pixel,
//! into a lit frame in [`LIT_FORMAT`] — light past one survives, so a lamp
//! can push a sprite into the bloom. Unlit — no ambient — the graph gets
//! none of it and the frame is the scene as drawn.
//!
//! Unity URP's 2D renderer and Godot's `CanvasModulate` + `PointLight2D`,
//! cut to what is built: point lights with a radius and a falloff, an
//! ambient colour, no shadows or normal maps yet.

use bytemuck::{Pod, Zeroable};
use glam::Vec2;
use wgpu::util::DeviceExt;

use crate::graph::{RenderGraph, Slot, TextureDesc};
use crate::shader;

/// The light map's and the lit frame's format: linear, with room past one.
pub const LIT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// A light shining from a point, fading to nothing at its radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointLight {
    /// Where, in the world.
    pub position: Vec2,
    /// Linear colour, multiplied by `intensity`.
    pub color: [f32; 3],
    pub intensity: f32,
    /// World units from the centre to where it gives nothing.
    pub radius: f32,
    /// How the light fades with distance: `1` is a straight ramp, `2` the
    /// soft default; higher keeps the light near the centre.
    pub falloff: f32,
}

impl PointLight {
    /// What the shader draws, or `None` for a light that gives nothing — no
    /// radius, no intensity, black, or a value that is not a number.
    fn instance(&self) -> Option<LightInstance> {
        let finite = self.position.is_finite()
            && self.color.iter().all(|c| c.is_finite())
            && self.intensity.is_finite()
            && self.radius.is_finite()
            && self.falloff.is_finite();
        let color = self.color.map(|c| c.max(0.0) * self.intensity);
        (finite && self.radius > 0.0 && self.intensity > 0.0 && color.iter().any(|&c| c > 0.0))
            .then(|| LightInstance {
                place: [
                    self.position.x,
                    self.position.y,
                    self.radius,
                    self.falloff.clamp(0.05, 16.0),
                ],
                color: [color[0], color[1], color[2], 0.0],
            })
    }
}

/// A frame's lighting: unlit without an ambient colour, lit by it and the
/// lights with one.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Lighting {
    /// The light everything gets, linear. `None` is unlit: the frame is the
    /// scene as drawn and no lighting pass runs.
    pub ambient: Option<[f32; 3]>,
    pub lights: Vec<PointLight>,
}

impl Lighting {
    /// The ambient colour, a non-number or negative channel as zero; `None`
    /// when unlit.
    fn ambient(&self) -> Option<wgpu::Color> {
        let channel = |c: f32| {
            if c.is_finite() {
                f64::from(c.max(0.0))
            } else {
                0.0
            }
        };
        self.ambient.map(|[r, g, b]| wgpu::Color {
            r: channel(r),
            g: channel(g),
            b: channel(b),
            a: 1.0,
        })
    }
}

/// One light as the vertex stage reads it. Must match `lights.wgsl`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
struct LightInstance {
    /// centre.xy, radius, falloff
    place: [f32; 4],
    /// colour × intensity, unused
    color: [f32; 4],
}

/// The two pipelines and the composite's layout. One per renderer.
pub struct LightPass {
    accumulate: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    composite_layout: wgpu::BindGroupLayout,
}

impl LightPass {
    /// Pipelines drawing lights through `camera_layout`, the world camera's.
    pub fn new(device: &wgpu::Device, camera_layout: &wgpu::BindGroupLayout) -> Self {
        let module = shader::create_module(device, "lights", shader::LIGHTS);
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lights-pipeline-layout"),
            bind_group_layouts: &[Some(camera_layout)],
            immediate_size: 0,
        });
        let additive = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::REPLACE,
        };
        let accumulate = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lights-accumulate"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_light"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<LightInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4],
                })],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_light"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: LIT_FORMAT,
                    blend: Some(additive),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let texture = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let composite_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lights-composite-layout"),
            entries: &[texture(0), texture(1)],
        });
        let module = shader::create_module(device, "lights-composite", shader::LIGHTS_COMPOSITE);
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lights-composite-pipeline-layout"),
            bind_group_layouts: &[Some(&composite_layout)],
            immediate_size: 0,
        });
        let composite = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lights-composite"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_full"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_composite"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: LIT_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            accumulate,
            composite,
            composite_layout,
        }
    }

    /// Adds the light map and the composite over `scene` — the drawn frame,
    /// `width` × `height` — to `graph`, `camera` bound for the lights.
    /// Returns the lit frame's slot, or `None` when `lighting` is unlit and
    /// the caller should read `scene` itself.
    #[allow(clippy::too_many_arguments)]
    pub fn add<'a>(
        &'a self,
        graph: &mut RenderGraph<'a>,
        device: &'a wgpu::Device,
        camera: &'a wgpu::BindGroup,
        scene: Slot,
        width: u32,
        height: u32,
        lighting: &Lighting,
    ) -> Option<Slot> {
        let ambient = lighting.ambient()?;
        let instances: Vec<LightInstance> = lighting
            .lights
            .iter()
            .filter_map(PointLight::instance)
            .collect();
        let count = u32::try_from(instances.len()).unwrap_or(u32::MAX);
        let buffer = (!instances.is_empty()).then(|| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("lights"),
                contents: bytemuck::cast_slice(&instances),
                usage: wgpu::BufferUsages::VERTEX,
            })
        });
        let desc = |label| TextureDesc {
            label,
            width,
            height,
            format: LIT_FORMAT,
        };
        let map = graph.create(desc("light-map"));
        let lit = graph.create(desc("lit-frame"));

        graph.add_pass("lights", &[], &[map], move |encoder, views| {
            let mut pass = begin(encoder, views.get(map), wgpu::LoadOp::Clear(ambient));
            if let Some(buffer) = &buffer {
                pass.set_pipeline(&self.accumulate);
                pass.set_bind_group(0, camera, &[]);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..6, 0..count);
            }
        });
        graph.add_pass(
            "lights-composite",
            &[scene, map],
            &[lit],
            move |encoder, views| {
                let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("lights-composite"),
                    layout: &self.composite_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(views.get(scene)),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(views.get(map)),
                        },
                    ],
                });
                let mut pass = begin(
                    encoder,
                    views.get(lit),
                    wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                );
                pass.set_pipeline(&self.composite);
                pass.set_bind_group(0, &group, &[]);
                pass.draw(0..3, 0..1);
            },
        );
        Some(lit)
    }
}

fn begin<'e>(
    encoder: &'e mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    load: wgpu::LoadOp<wgpu::Color>,
) -> wgpu::RenderPass<'e> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("lights-pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load,
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lamp() -> PointLight {
        PointLight {
            position: Vec2::ZERO,
            color: [1.0, 0.5, 0.0],
            intensity: 2.0,
            radius: 3.0,
            falloff: 2.0,
        }
    }

    #[test]
    fn a_light_is_its_colour_times_its_intensity() {
        let i = lamp().instance().expect("gives light");
        assert_eq!(i.color[..3], [2.0, 1.0, 0.0]);
        assert_eq!(i.place, [0.0, 0.0, 3.0, 2.0]);
    }

    #[test]
    fn a_light_that_gives_nothing_is_not_drawn() {
        for dark in [
            PointLight {
                radius: 0.0,
                ..lamp()
            },
            PointLight {
                intensity: 0.0,
                ..lamp()
            },
            PointLight {
                color: [0.0; 3],
                ..lamp()
            },
            PointLight {
                position: Vec2::NAN,
                ..lamp()
            },
        ] {
            assert_eq!(dark.instance(), None, "{dark:?}");
        }
    }

    #[test]
    fn unlit_is_no_ambient_and_nonsense_ambient_is_black() {
        assert_eq!(Lighting::default().ambient(), None);
        let odd = Lighting {
            ambient: Some([f32::NAN, -1.0, 0.5]),
            lights: Vec::new(),
        };
        let c = odd.ambient().expect("lit");
        assert_eq!((c.r, c.g, c.b), (0.0, 0.0, 0.5));
    }
}
