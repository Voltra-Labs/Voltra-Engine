//! Frame orchestration: what gets recorded and submitted each tick.

use wgpu::SurfaceTarget;

use crate::bloom::BloomPass;
use crate::camera::{CameraBinding, View2D};
use crate::context::GpuContext;
use crate::graph::TexturePool;
use crate::light_layers::LayerPass;
use crate::lights::{LightPass, Lighting};
use crate::lines::ViewportBinding;
use crate::normals::NormalPass;
use crate::particles::ParticlePipelines;
use crate::post::PostPass;
use crate::target::RenderTarget;
use crate::texture::Texture;
use crate::{pipeline, texture};

mod access;
mod composite;
mod direct;
mod frame;
mod gpu_timing;
mod layer;
mod light_layers;
mod normals;
mod post_chain;
mod world3d;

pub use composite::Placed;
pub use layer::Layer;

/// Drives one frame at a time on top of a [`GpuContext`].
pub struct Renderer {
    ctx: GpuContext,
    flat_color: crate::pipeline::SpritePipeline,
    camera_binding: CameraBinding,
    /// The screen-space UI's projection, in its own buffer so it and the
    /// world camera can both be written before one submit.
    ui_camera_binding: CameraBinding,
    /// Layout group 1 was built against. Exposed so a texture cache outside
    /// this crate can build bind groups that actually match the pipeline —
    /// a second layout with the same descriptor does not.
    texture_layout: wgpu::BindGroupLayout,
    /// Bound whenever geometry has no texture of its own. White multiplies to
    /// a no-op, so this is what lets one pipeline serve both cases.
    white_bind_group: wgpu::BindGroup,
    /// Draws overlay segments over an already-rendered target.
    lines: wgpu::RenderPipeline,
    /// Grades the drawn scene into the frame.
    post: PostPass,
    /// Spreads bright light into a glow, when the grade asks for one.
    bloom: BloomPass,
    /// Lights the drawn scene, when the frame is lit.
    lights: LightPass,
    /// Draws the scene's normals, when a light shades normal maps.
    normals: NormalPass,
    /// Where the normals are drawn, sized to the scene; made the first frame
    /// that needs it.
    normal_target: Option<RenderTarget>,
    /// Draws the scene's light layers, when a light leaves some out.
    layers: LayerPass,
    /// Where the light layers are drawn, sized to the scene; made the first
    /// frame that needs it.
    layer_target: Option<RenderTarget>,
    /// The render graph's per-frame textures, kept between frames.
    transients: TexturePool,
    /// Times the passes on the GPU while asked to; see `gpu_timing.rs`.
    gpu_timer: crate::gpu_timer::GpuTimer,
    /// Steps and draws GPU particles; `None` on an adapter that cannot.
    particles: Option<ParticlePipelines>,
    /// Draws sprite meshes deformed by bones; every adapter can.
    skin: crate::skinning::SkinPipeline,
    /// Where the world is drawn before the post pass on the window path,
    /// sized to the surface; made on the first frame.
    frame_target: Option<RenderTarget>,
    /// The 3D pass and its targets, made with the first 3D frame: a 2D
    /// game never builds them.
    state3d: world3d::State3D,
    /// The composite's pipeline, blending pictures premultiplied; made
    /// with the first frame of several cameras.
    composite: std::sync::OnceLock<crate::pipeline::SpritePipeline>,
    /// The size the line shader widens against, rewritten every frame.
    viewport_binding: ViewportBinding,
    /// The world camera, rewritten into its uniform every frame.
    pub camera: View2D,
    /// What the next frame is lit by; unlit by default. Set before each
    /// frame, as [`camera`](Self::camera) is.
    pub lighting: Lighting,
    /// The colour the frame is cleared to before anything is drawn.
    pub clear_color: wgpu::Color,
}

impl Renderer {
    /// Blocks on device creation; native only — see [`GpuContext::new`].
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(target: impl Into<SurfaceTarget<'static>>, width: u32, height: u32) -> Self {
        Self::with_context(GpuContext::new(target, width, height), width, height)
    }

    /// [`Renderer::new`], awaited: the web's way, where nothing may block.
    pub async fn new_async(
        target: impl Into<SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Self {
        let ctx = GpuContext::new_async(target, width, height).await;
        Self::with_context(ctx, width, height)
    }

    /// A renderer with no window, drawing only into offscreen targets —
    /// [`render_scene`](Self::render_scene) and its kin. `None` without a
    /// usable adapter.
    pub fn headless(width: u32, height: u32) -> Option<Self> {
        Some(Self::with_context(
            GpuContext::headless(width, height)?,
            width,
            height,
        ))
    }

    fn with_context(ctx: GpuContext, width: u32, height: u32) -> Self {
        let camera_binding = CameraBinding::new(ctx.device());
        let ui_camera_binding = camera_binding.sharing_layout(ctx.device());
        let texture_layout = texture::bind_group_layout(ctx.device());
        let flat_color = pipeline::create_flat_color(
            ctx.device(),
            ctx.format(),
            camera_binding.layout(),
            &texture_layout,
        );
        let white = Texture::white(ctx.device(), ctx.queue());
        let white_bind_group = white.create_bind_group(ctx.device(), &texture_layout);

        let viewport_binding = ViewportBinding::new(ctx.device());
        let lines = pipeline::create_lines(
            ctx.device(),
            ctx.format(),
            camera_binding.layout(),
            viewport_binding.layout(),
        );

        let post = PostPass::new(ctx.device(), ctx.format());
        let bloom = BloomPass::new(ctx.device());
        let lights = LightPass::new(ctx.device(), camera_binding.layout(), &texture_layout);
        let normals = NormalPass::new(
            ctx.device(),
            ctx.queue(),
            camera_binding.layout(),
            &texture_layout,
        );
        let particles = if ParticlePipelines::supported(ctx.downlevel()) {
            Some(ParticlePipelines::new(
                ctx.device(),
                ctx.format(),
                camera_binding.layout(),
                &texture_layout,
            ))
        } else {
            log::warn!("this adapter has no compute shaders; GPU particles run on the CPU");
            None
        };
        let skin = crate::skinning::SkinPipeline::new(
            ctx.device(),
            ctx.format(),
            camera_binding.layout(),
            &texture_layout,
        );
        let layers = LayerPass::new(
            ctx.device(),
            camera_binding.layout(),
            &texture_layout,
            particles.as_ref(),
        );

        let camera = {
            let mut camera = View2D::default();
            camera.aspect = aspect_of(width, height);
            camera
        };

        let gpu_timer = crate::gpu_timer::GpuTimer::new(ctx.device(), ctx.queue());

        Self {
            ctx,
            flat_color,
            camera_binding,
            ui_camera_binding,
            texture_layout,
            white_bind_group,
            lines,
            post,
            bloom,
            lights,
            normals,
            normal_target: None,
            layers,
            layer_target: None,
            transients: TexturePool::default(),
            gpu_timer,
            particles,
            skin,
            frame_target: None,
            state3d: world3d::State3D::default(),
            composite: std::sync::OnceLock::new(),
            viewport_binding,
            camera,
            lighting: Lighting::default(),
            clear_color: wgpu::Color {
                r: 0.1,
                g: 0.1,
                b: 0.12,
                a: 1.0,
            },
        }
    }
}

/// Guards against the divide-by-zero a minimised window would otherwise cause.
fn aspect_of(width: u32, height: u32) -> f32 {
    width.max(1) as f32 / height.max(1) as f32
}
