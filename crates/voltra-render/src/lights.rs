//! 2D lights, as passes of the frame's [`RenderGraph`].
//!
//! A light map the frame's size is cleared to the ambient colour; each
//! point light is added onto it as a quad around the light, through the
//! world camera; the drawn scene is then multiplied by it, pixel for pixel,
//! into a lit frame in [`LIT_FORMAT`] — light past one survives, so a lamp
//! can push a sprite into the bloom. Unlit — no ambient — the graph gets
//! none of it and the frame is the scene as drawn.
//!
//! A light that casts shadows reads them from a shadow map drawn just
//! before the light map, from the frame's [`Occluder`]s — see [`shadows`].
//!
//! Unity URP's 2D renderer and Godot's `CanvasModulate` + `PointLight2D` +
//! `LightOccluder2D`, cut to what is built: point lights with a radius and a
//! falloff, an ambient colour, shadows from outlines and lines, no normal
//! maps yet.

use std::sync::atomic::{AtomicBool, Ordering};

use wgpu::util::DeviceExt;

use crate::graph::{RenderGraph, Slot, TextureDesc};
use crate::shader;

mod instance;
mod lighting;
mod occluder;
pub mod shadows;

use instance::LightInstance;
pub use lighting::{
    Lighting, PointLight, Shadow, DEFAULT_SHADOW_RESOLUTION, LIT_FORMAT, MAX_SHADOW_SOFTNESS,
};
use occluder::Edges;
pub use occluder::Occluder;
use shadows::{ShadowPass, ShadowPlan, SHADOW_FORMAT};

/// The pipelines, the composite's layout and the shadow map's pass. One per
/// renderer.
pub struct LightPass {
    accumulate: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    composite_layout: wgpu::BindGroupLayout,
    shadows: ShadowPass,
    /// Whether the shadow map running out of rows has been reported since it
    /// last had enough: said when it starts, not every frame it lasts.
    overflow_reported: AtomicBool,
}

impl LightPass {
    /// Pipelines drawing lights through `camera_layout`, the world camera's.
    pub fn new(device: &wgpu::Device, camera_layout: &wgpu::BindGroupLayout) -> Self {
        let shadows = ShadowPass::new(device);
        let module = shader::create_module(device, "lights", shader::LIGHTS);
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lights-pipeline-layout"),
            bind_group_layouts: &[Some(camera_layout), Some(shadows.layout())],
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
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x4,
                        1 => Float32x4,
                        2 => Float32x4,
                    ],
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
            shadows,
            overflow_reported: AtomicBool::new(false),
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
        let plan = ShadowPlan::new(
            &lighting.lights,
            &Edges::new(&lighting.occluders),
            lighting.shadow_resolution,
            device.limits().max_texture_dimension_2d,
        );
        // Said when it starts, not every frame it lasts; once it clears, the
        // next time is said again.
        if plan.overflow == 0 {
            self.overflow_reported.store(false, Ordering::Relaxed);
        } else if !self.overflow_reported.swap(true, Ordering::Relaxed) {
            log::warn!(
                "{} shadowed light(s) drawn without shadows: the shadow map has no rows left",
                plan.overflow
            );
        }
        let instances: Vec<LightInstance> = lighting
            .lights
            .iter()
            .zip(&plan.rows)
            .filter_map(|(light, row)| light.instance(*row))
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

        let edges = self.shadows.upload(device, &plan);
        let shadow_map = edges.map(|edges| {
            let slot = graph.create(TextureDesc {
                label: "light-shadows",
                width: plan.width,
                height: plan.height,
                format: SHADOW_FORMAT,
            });
            let count = shadows::instance_count(&plan);
            graph.add_pass("light-shadows", &[], &[slot], move |encoder, views| {
                self.shadows.record(encoder, views.get(slot), &edges, count);
            });
            slot
        });
        let reads: Vec<Slot> = shadow_map.into_iter().collect();
        graph.add_pass("lights", &reads, &[map], move |encoder, views| {
            let group = shadow_map.map(|slot| self.shadows.bind_map(device, views.get(slot)));
            let mut pass = begin(encoder, views.get(map), wgpu::LoadOp::Clear(ambient));
            if let Some(buffer) = &buffer {
                pass.set_pipeline(&self.accumulate);
                pass.set_bind_group(0, camera, &[]);
                pass.set_bind_group(1, group.as_ref().unwrap_or(self.shadows.empty()), &[]);
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
