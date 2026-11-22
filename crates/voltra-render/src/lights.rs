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
//! `LightOccluder2D`: point and spot lights with a radius and a falloff, an
//! ambient colour, shadows from outlines and lines, normal maps and light
//! layers.

use std::sync::atomic::AtomicBool;

use crate::shader;

mod global;
mod instance;
mod layer_input;
mod lighting;
mod normal_input;
mod occluder;
mod passes;
mod reach;
pub mod shadows;

use global::GlobalInstance;
pub use global::GlobalLight;
use instance::LightInstance;
use layer_input::LayerInput;
pub use lighting::{
    Cone, Lighting, PointLight, Shadow, ALL_LIGHT_LAYERS, DEFAULT_LIGHT_LAYERS,
    DEFAULT_SHADOW_RESOLUTION, LIT_FORMAT, MAX_SHADOW_SOFTNESS,
};
use normal_input::NormalInput;
pub use occluder::Occluder;
pub use reach::Reach;
use shadows::ShadowPass;

/// The pipelines, the composite's layout and the shadow map's pass. One per
/// renderer.
pub struct LightPass {
    accumulate: wgpu::RenderPipeline,
    /// Global lights, a full-screen triangle each.
    global: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    composite_layout: wgpu::BindGroupLayout,
    shadows: ShadowPass,
    normals: NormalInput,
    layers: LayerInput,
    /// Whether the shadow map running out of rows has been reported since it
    /// last had enough: said when it starts, not every frame it lasts.
    overflow_reported: AtomicBool,
}

impl LightPass {
    /// Pipelines drawing lights through `camera_layout`, the world camera's.
    pub fn new(device: &wgpu::Device, camera_layout: &wgpu::BindGroupLayout) -> Self {
        let shadows = ShadowPass::new(device);
        let normals = NormalInput::new(device);
        let layers = LayerInput::new(device);
        let module = shader::create_module(device, "lights", shader::LIGHTS);
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lights-pipeline-layout"),
            bind_group_layouts: &[
                Some(camera_layout),
                Some(shadows.layout()),
                Some(normals.layout()),
                Some(layers.layout()),
            ],
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
                        3 => Float32x4,
                        4 => Float32x4,
                        5 => Uint32,
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

        let global = global_pipeline(device, layers.layout(), additive);

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
            global,
            composite,
            composite_layout,
            shadows,
            normals,
            layers,
            overflow_reported: AtomicBool::new(false),
        }
    }
}

/// The global lights' pipeline: `lights_global.wgsl`, added onto the light
/// map with `blend`, reading the scene's light layers through `layers`.
fn global_pipeline(
    device: &wgpu::Device,
    layers: &wgpu::BindGroupLayout,
    blend: wgpu::BlendState,
) -> wgpu::RenderPipeline {
    let module = shader::create_module(device, "lights-global", shader::LIGHTS_GLOBAL);
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("lights-global-layout"),
        bind_group_layouts: &[Some(layers)],
        immediate_size: 0,
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("lights-global"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs_global"),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<GlobalInstance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Uint32],
            })],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs_global"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: LIT_FORMAT,
                blend: Some(blend),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}
