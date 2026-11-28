use voltra_assets::{StandardFile, SurfaceSource};
use voltra_render::read_rgba8;

use super::*;

fn gold() -> SurfaceSource {
    StandardFile {
        base_color: [1.0, 0.77, 0.34, 1.0],
        metallic: 1.0,
        roughness: 0.3,
        ..StandardFile::default()
    }
    .resolve()
    .expect("a surface")
}

#[test]
fn a_preview_is_drawn_once_lit_on_a_sphere_and_again_when_it_changes() {
    let Some(mut renderer) = Renderer::headless(64, 64) else {
        eprintln!("no adapter: skipping");
        return;
    };
    let root = voltra_testkit::scratch_root();
    let device = renderer.context().device().clone();
    let queue = renderer.context().queue().clone();
    let mut textures = Textures::new(&device, &queue, renderer.texture_layout(), root.path());
    let mut meshes = Meshes::new();
    let mut previews = SurfacePreviews::default();
    let mut ids = 0;
    let mut register = |_: &RenderTarget| {
        ids += 1;
        TextureId::User(ids)
    };
    let freed = std::cell::RefCell::new(Vec::new());
    let mut free = |id: TextureId| freed.borrow_mut().push(id);
    let size = 96;
    assert_eq!(
        previews.ask("gold", size, &gold()),
        None,
        "no id the first frame"
    );
    let drawn = previews.draw(
        &mut renderer,
        &mut textures,
        &mut meshes,
        &mut register,
        &mut free,
    );
    assert_eq!(drawn, 1);
    assert_eq!(
        previews.ask("gold", size, &gold()),
        Some(TextureId::User(1))
    );
    assert_eq!(
        previews.draw(
            &mut renderer,
            &mut textures,
            &mut meshes,
            &mut register,
            &mut free
        ),
        0,
        "unchanged: not drawn again"
    );
    let target = previews.target("gold", size).expect("drawn");
    let rgba = read_rgba8(&device, &queue, target.texture(), size, size);
    if let Some(dir) = std::env::var_os("VOLTRA_SHOT_DIR") {
        image::save_buffer(
            std::path::Path::new(&dir).join("surface-preview.png"),
            &rgba,
            size,
            size,
            image::ColorType::Rgba8,
        )
        .expect("the picture is written");
    }
    let at = |x: u32, y: u32| {
        let i = ((y * size + x) * 4) as usize;
        [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
    };
    // The corner is transparent, for the panel to show through; the
    // sphere is opaque and gold, brighter toward the light, upper left.
    assert_eq!(at(2, 2)[3], 0, "{:?}", at(2, 2));
    let middle = at(size / 2, size / 2);
    assert_eq!(middle[3], 255);
    let lit = (size / 2 - size / 6..size / 2)
        .flat_map(|y| (size / 2 - size / 6..size / 2).map(move |x| (x, y)))
        .map(|(x, y)| at(x, y)[0])
        .max()
        .unwrap_or(0);
    let shade = at(size / 2 + size / 4, size / 2 + size / 4);
    assert!(lit > shade[0] + 60, "lit {lit}, shade {shade:?}");
    assert!(middle[0] >= middle[2], "gold, not blue: {middle:?}");
    // Rougher: drawn again.
    let rough = SurfaceSource {
        standard: voltra_render::render3d::StandardSurface {
            roughness: 0.9,
            ..gold().standard
        },
        ..gold()
    };
    previews.ask("gold", size, &rough);
    assert_eq!(
        previews.draw(
            &mut renderer,
            &mut textures,
            &mut meshes,
            &mut register,
            &mut free
        ),
        1
    );
    // No longer asked for: kept a while, then let go with its id.
    for _ in 0..KEEP_DRAWS {
        previews.draw(
            &mut renderer,
            &mut textures,
            &mut meshes,
            &mut register,
            &mut free,
        );
    }
    assert!(
        previews.target("gold", size).is_some(),
        "kept {KEEP_DRAWS} draws"
    );
    assert!(freed.borrow().is_empty());
    previews.draw(
        &mut renderer,
        &mut textures,
        &mut meshes,
        &mut register,
        &mut free,
    );
    assert!(previews.target("gold", size).is_none(), "let go");
    assert_eq!(*freed.borrow(), [TextureId::User(1)]);
}
