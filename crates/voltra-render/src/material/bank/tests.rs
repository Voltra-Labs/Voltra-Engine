//! The sprite material bank on a real device: a shader the GPU refuses is
//! caught in its validation scope and its sprites draw plainly.

use std::collections::BTreeMap;

use super::*;
use crate::material::{compose, Layout};

#[test]
fn a_shader_the_gpu_refuses_draws_plainly_and_is_said_once() {
    let Some(renderer) = crate::Renderer::headless(16, 16) else {
        eprintln!("no adapter: skipping");
        return;
    };
    let device = renderer.context().device();
    let queue = renderer.context().queue();
    let format = renderer.context().format();
    let layout = Layout::new(&BTreeMap::new()).expect("nothing fits");
    // Valid WGSL binding what the pipeline's layout does not have: a
    // stand-in for what only a browser's compiler refuses, which no native
    // check can make.
    let refused = compose(
        "@group(2) @binding(3) var<uniform> extra: vec4<f32>;\nfn fragment(in: Fragment) -> vec4<f32> { return sprite_color(in) * extra; }",
        &layout,
    );
    let fine = compose(
        "fn fragment(in: Fragment) -> vec4<f32> { return sprite_color(in); }",
        &layout,
    );
    let mut bank = MaterialBank::new(device);
    let requests = [
        Some(MaterialRequest {
            revision: 1,
            source: &refused,
            block: [0; BLOCK_SIZE],
        }),
        Some(MaterialRequest {
            revision: 2,
            source: &fine,
            block: [0; BLOCK_SIZE],
        }),
    ];
    let layouts = [renderer.camera_layout(), renderer.texture_layout()];
    bank.prepare(device, queue, format, layouts, &requests);
    assert!(bank.is_refused(1), "caught in its scope");
    assert!(bank.draw(0).is_none(), "its sprites draw plainly");
    assert!(!bank.is_refused(2));
    assert!(bank.draw(1).is_some(), "the other draws through its own");
    assert_eq!(renderer.context().uncaptured_errors(), 0);
    // The next frame neither makes it again nor says it again.
    let held = bank.len();
    bank.prepare(device, queue, format, layouts, &requests);
    assert_eq!(bank.len(), held);
    assert!(bank.draw(0).is_none());
}
