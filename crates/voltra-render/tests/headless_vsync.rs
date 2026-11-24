//! Vsync is the surface's present mode: on from the start, off and on again
//! by asking, and what was asked is what is answered.

use voltra_render::context::GpuContext;
use voltra_render::wgpu;

#[test]
fn vsync_starts_on_and_follows_what_is_asked() {
    let Some(mut ctx) = GpuContext::headless(64, 64) else {
        eprintln!("no GPU adapter available; skipping");
        return;
    };
    assert!(ctx.vsync(), "a context starts with vsync on");
    assert_eq!(ctx.config().present_mode, wgpu::PresentMode::Fifo);

    ctx.set_vsync(false);
    assert!(!ctx.vsync());
    assert_eq!(ctx.config().present_mode, wgpu::PresentMode::AutoNoVsync);

    ctx.set_vsync(true);
    assert!(ctx.vsync());
    assert_eq!(ctx.config().present_mode, wgpu::PresentMode::Fifo);
}
