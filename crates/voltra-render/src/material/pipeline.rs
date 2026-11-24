//! A material's pipeline: the sprite pipeline with the material's fragment
//! stage and its uniforms at group 2.

use crate::pipeline::{SpritePipeline, SpritePipelineDescriptor};
use crate::shader;

/// The layout of group 2: one uniform block, bound at a dynamic offset.
pub fn uniform_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("material-uniforms-layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: None,
            },
            count: None,
        }],
    })
}

/// The pipelines drawing sprites — triangles and quads — through the [`compose`](super::compose)d
/// `source`, into `format`, blended as the sprite pipeline blends. The
/// source must have passed [`validate`](super::validate).
pub fn create_material_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    label: &str,
    source: &str,
    layouts: [&wgpu::BindGroupLayout; 3],
) -> SpritePipeline {
    let module = shader::create_module(device, label, source);
    let [camera, texture, uniforms] = layouts;
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[Some(camera), Some(texture), Some(uniforms)],
        immediate_size: 0,
    });
    SpritePipeline::new(
        device,
        &SpritePipelineDescriptor {
            label,
            layout: &layout,
            module: &module,
            triangles_entry: "vs_main",
            quads_entry: "vs_quad",
            fragment_entry: "fs_main",
            target: wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            },
        },
    )
}
