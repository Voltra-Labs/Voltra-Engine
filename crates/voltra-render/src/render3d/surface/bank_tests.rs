//! The bank on a real device: a shader the GPU refuses is caught in its
//! validation scope, marked broken and drawn as the standard.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::*;
use crate::material::Layout;
use crate::render3d::Winding;

fn key(shader: &SurfaceShader) -> SurfaceKey {
    SurfaceKey {
        shader: shader.revision(),
        vertex: SurfaceVertex::Plain,
        alpha: AlphaClass::Opaque,
        double_sided: false,
        winding: Winding::CounterClockwise,
        pass: SurfacePass::Main,
    }
}

#[test]
fn a_shader_the_gpu_refuses_is_marked_broken_and_drawn_as_the_standard() {
    let Some(gpu) = crate::GpuContext::headless(16, 16) else {
        eprintln!("no adapter: skipping");
        return;
    };
    let (device, queue) = (gpu.device(), gpu.queue());
    let layouts = SurfaceLayouts::new(device, queue);
    let mut bank = SurfaceBank::new(device, &layouts, wgpu::TextureFormat::Rgba16Float, 1);
    // Valid WGSL binding what the pipeline's layout does not have: what
    // `SurfaceShader::new` refuses, made here unchecked, as a stand-in for
    // what only a browser's compiler refuses.
    let layout = Layout::new(&BTreeMap::new()).expect("nothing fits");
    let source = "@group(1) @binding(12) var extra: texture_2d<f32>;\nfn surface(in: SurfaceInput) -> Surface {\n    var s = standard_surface(in);\n    s.base_color = textureSample(extra, surface_sampler, in.uv).rgb;\n    return s;\n}\n";
    let composed = compose_surface(source, &layout, &[], SurfaceLighting::Lit).expect("composes");
    let refused = Arc::new(SurfaceShader::unchecked(composed));
    let standard = SurfaceShader::standard();
    bank.begin();
    bank.prepare(device, key(&refused), &refused);
    assert!(bank.is_broken(refused.revision()), "caught in its scope");
    assert!(!bank.is_broken(standard.revision()));
    let drawn = bank.pipeline(key(&refused)).expect("a pipeline still");
    assert!(
        Some(drawn) == bank.pipeline(key(&standard)),
        "its draws use the standard's"
    );
    assert_eq!(
        gpu.uncaptured_errors(),
        0,
        "the scope took it, not the handler"
    );
    // Asked again, it is not made again.
    let made = bank.len();
    bank.begin();
    bank.prepare(device, key(&refused), &refused);
    assert_eq!(bank.len(), made);
    // Let go by its materials, its mark goes with it.
    let revision = refused.revision();
    drop(refused);
    bank.begin();
    assert!(!bank.is_broken(revision));
}
