//! A renderer with no window drawing into an offscreen target, read back.

use voltra_render::{read_rgba8, Filter, Layer, PostSettings, RenderTarget, Renderer};

#[test]
fn a_headless_renderer_clears_a_target_that_reads_back_as_its_clear_colour() {
    let Some(mut renderer) = Renderer::headless(32, 16) else {
        eprintln!("no adapter: skipping");
        return;
    };
    assert!(!renderer.context().has_surface());
    let device = renderer.context().device().clone();
    let queue = renderer.context().queue().clone();
    let format = renderer.context().format();
    let target = RenderTarget::new(&device, "t", format, 32, 16, Filter::Linear);
    let graded = RenderTarget::new(&device, "g", format, 32, 16, Filter::Linear);
    renderer.clear_color = voltra_render::wgpu::Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    renderer.render_scene(
        &target,
        &graded,
        None,
        &[],
        Layer::EMPTY,
        &PostSettings::default(),
    );
    let pixels = read_rgba8(&device, &queue, graded.texture(), 32, 16);
    assert_eq!(pixels.len(), 32 * 16 * 4);
    for pixel in pixels.as_chunks::<4>().0 {
        assert!(
            pixel[0] > 240 && pixel[1] < 16 && pixel[2] < 16,
            "red, in RGBA order: {pixel:?}"
        );
    }
}
