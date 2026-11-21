//! The light passes a frame's graph gets: the shadow map, the light map and
//! the composite.

use std::sync::atomic::Ordering;

use wgpu::util::DeviceExt;

use super::instance::LightInstance;
use super::occluder::Edges;
use super::shadows::{self, ShadowPlan, SHADOW_FORMAT};
use super::{LightPass, Lighting, LIT_FORMAT};
use crate::graph::{RenderGraph, Slot, TextureDesc};

impl LightPass {
    /// Adds the light map and the composite over `scene` — the drawn frame,
    /// `width` × `height` — to `graph`, `camera` bound for the lights.
    /// `normals` is the scene's normal buffer (the frame's size), for lights
    /// with a height; `None` shades every light flat.
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
        normals: Option<&'a wgpu::TextureView>,
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
            let surface = normals.map(|view| self.normals.bind(device, view));
            let mut pass = begin(encoder, views.get(map), wgpu::LoadOp::Clear(ambient));
            if let Some(buffer) = &buffer {
                pass.set_pipeline(&self.accumulate);
                pass.set_bind_group(0, camera, &[]);
                pass.set_bind_group(1, group.as_ref().unwrap_or(self.shadows.empty()), &[]);
                pass.set_bind_group(2, surface.as_ref().unwrap_or(self.normals.empty()), &[]);
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
