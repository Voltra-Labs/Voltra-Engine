//! The light passes a frame's graph gets: the shadow map, the light map and
//! the composite.

use std::sync::atomic::Ordering;

use super::global::GlobalInstance;
use super::instance::LightInstance;
use super::occluder::Edges;
use super::shadows::{self, ShadowPlan, SHADOW_FORMAT};
use super::{GlobalLight, LightPass, Lighting, LIT_FORMAT};
use crate::frame_buffers::FrameBuffers;
use crate::graph::{RenderGraph, Slot, TextureDesc};

impl LightPass {
    /// Adds the light map and the composite over `scene` — the drawn frame,
    /// `width` × `height` — to `graph`, `camera` bound for the lights.
    /// `normals` is the scene's normal buffer (the frame's size), for lights
    /// with a height; `None` shades every light flat. `layers` is the scene's
    /// light-layer buffer, for lights that leave a layer out; `None` puts
    /// every pixel on every layer.
    /// Returns the lit frame's slot, or `None` when `lighting` is unlit and
    /// the caller should read `scene` itself.
    #[allow(clippy::too_many_arguments)]
    pub fn add<'a>(
        &'a self,
        graph: &mut RenderGraph<'a>,
        device: &'a wgpu::Device,
        buffers: &FrameBuffers,
        camera: &'a wgpu::BindGroup,
        scene: Slot,
        width: u32,
        height: u32,
        lighting: &Lighting,
        normals: Option<&'a wgpu::TextureView>,
        layers: Option<&'a wgpu::TextureView>,
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
        // Grouped by the cookie each light wears, so a run of them shares one
        // binding; the order lights are added in does not matter.
        let cookies = lighting.cookies.len();
        let mut worn: Vec<(Option<u32>, LightInstance)> = lighting
            .lights
            .iter()
            .zip(&plan.rows)
            .filter_map(|(light, row)| {
                light
                    .instance(*row, cookies)
                    .map(|instance| (light.cookie_index(cookies), instance))
            })
            .collect();
        worn.sort_by_key(|(cookie, _)| *cookie);
        let runs = cookie_runs(&worn);
        let instances: Vec<LightInstance> = worn.into_iter().map(|(_, i)| i).collect();
        // Cloned into the pass, which outlives the borrow of `lighting`: a
        // bind group is reference-counted.
        let lighting_cookies = lighting.cookies.clone();
        let buffer = (!instances.is_empty()).then(|| {
            buffers.create(
                device,
                &wgpu::util::BufferInitDescriptor {
                    label: Some("lights"),
                    contents: bytemuck::cast_slice(&instances),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            )
        });
        let globals: Vec<GlobalInstance> = lighting
            .globals
            .iter()
            .filter_map(GlobalLight::instance)
            .collect();
        let global_count = u32::try_from(globals.len()).unwrap_or(u32::MAX);
        let global_buffer = (!globals.is_empty()).then(|| {
            buffers.create(
                device,
                &wgpu::util::BufferInitDescriptor {
                    label: Some("global-lights"),
                    contents: bytemuck::cast_slice(&globals),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            )
        });
        let desc = |label| TextureDesc {
            label,
            width,
            height,
            format: LIT_FORMAT,
        };
        let map = graph.create(desc("light-map"));
        let lit = graph.create(desc("lit-frame"));

        let edges = self.shadows.upload(device, buffers, &plan);
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
            let surface = self.scene.bind(device, normals, layers);
            let mut pass = begin(encoder, views.get(map), wgpu::LoadOp::Clear(ambient));
            if let Some(buffer) = &global_buffer {
                pass.set_pipeline(&self.global);
                pass.set_bind_group(0, &surface, &[]);
                pass.set_vertex_buffer(0, buffer.slice(..));
                pass.draw(0..3, 0..global_count);
            }
            if let Some(buffer) = &buffer {
                pass.set_pipeline(&self.accumulate);
                pass.set_bind_group(0, camera, &[]);
                pass.set_bind_group(1, group.as_ref().unwrap_or(self.shadows.empty()), &[]);
                pass.set_bind_group(2, &surface, &[]);
                pass.set_vertex_buffer(0, buffer.slice(..));
                for (cookie, range) in &runs {
                    let image = cookie
                        .and_then(|index| lighting_cookies.get(index as usize))
                        .unwrap_or(&self.no_cookie);
                    pass.set_bind_group(3, image, &[]);
                    pass.draw(0..6, range.clone());
                }
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

/// The instance ranges of `worn`, sorted by cookie, that share one.
fn cookie_runs(worn: &[(Option<u32>, LightInstance)]) -> Vec<(Option<u32>, std::ops::Range<u32>)> {
    let mut runs: Vec<(Option<u32>, std::ops::Range<u32>)> = Vec::new();
    for (index, (cookie, _)) in worn.iter().enumerate() {
        // Instances are counted in the thousands at most.
        let index = u32::try_from(index).unwrap_or(u32::MAX);
        match runs.last_mut() {
            Some((last, range)) if last == cookie => range.end = index + 1,
            _ => runs.push((*cookie, index..index + 1)),
        }
    }
    runs
}
