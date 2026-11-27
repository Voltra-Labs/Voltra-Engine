//! A pause menu, the whole way: a 9-sliced panel stacking a title and three
//! image buttons in a column, written to a scene file and read back, drawn
//! in pixels, and clicked.
//!
//! The skin is one sheet of five 16 × 16 cells — the button's normal,
//! hovered, pressed and disabled faces, and the panel — each a 4-texel white
//! (yellow for the panel) frame around a coloured middle. Drawn sliced, a
//! frame stays 4 points thick however big the element; stretched whole, it
//! would be a quarter of it. The pixels tell the two apart.
//!
//! Skips the drawing when no GPU adapter is available; the layout and the
//! clicks are checked either way.

use std::path::Path;

use voltra_assets::{AssetPath, Atlases, Fonts, Textures, DEFAULT_FONT};
use voltra_ecs::{Entity, World};
use voltra_render::camera::{screen_projection, CameraBinding, View2D};
use voltra_render::glam::Vec2;
use voltra_render::pass::{self, MeshDraw};
use voltra_render::{pipeline, texture, wgpu, Texture};
use voltra_scene::format::{load, save};
use voltra_scene::sprite::sheets::Sheets;
use voltra_scene::text::{prepare, resolve_fonts};
use voltra_scene::ui::button::ButtonFrames;
use voltra_scene::ui::stack::{Align as StackAlign, Direction};
use voltra_scene::ui::{Edges, Navigation, Pointer, UiInput};
use voltra_scene::{
    hierarchy, ui, Align, ComponentRegistry, Name, SceneId, Sprite, SpriteBatch, Text, UiButton,
    UiControls, UiLayout, UiRect, UiSlice, UiStack,
};
use voltra_testkit::{copy_bundled_font, headless_device, read_texture, scratch_root, Rgba, CLEAR};

const SIZE: u32 = 200;
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const SCREEN: Vec2 = Vec2::new(SIZE as f32, SIZE as f32);

const WHITE: [u8; 4] = [255, 255, 255, 255];
const YELLOW: [u8; 4] = [255, 255, 0, 255];
/// The middles: normal, hovered, pressed, disabled, panel.
const MIDDLES: [[u8; 4]; 5] = [
    [0, 0, 255, 255],
    [0, 255, 0, 255],
    [255, 0, 0, 255],
    [128, 128, 128, 255],
    [255, 0, 255, 255],
];
const PANEL_FRAME: u32 = 4;

/// The skin: five cells in a row, and the atlas cutting it.
fn write_skin(root: &Path) {
    use image::ImageEncoder;

    let (width, height) = (16 * 5, 16);
    let mut pixels = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            let cell = x / 16;
            let (cx, cy) = (x % 16, y);
            let border = !(4..12).contains(&cx) || !(4..12).contains(&cy);
            let rgba = match (border, cell as u32 == PANEL_FRAME) {
                (true, true) => YELLOW,
                (true, false) => WHITE,
                (false, _) => MIDDLES[cell],
            };
            pixels.extend_from_slice(&rgba);
        }
    }
    let file = std::fs::File::create(root.join("skin.png")).expect("the skin");
    image::codecs::png::PngEncoder::new(std::io::BufWriter::new(file))
        .write_image(
            &pixels,
            width as u32,
            height as u32,
            image::ExtendedColorType::Rgba8,
        )
        .expect("encoding the skin");
    std::fs::write(
        root.join("skin.atlas.ron"),
        b"(version: 1, texture: Some(\"skin.png\"), grid: Some((cell: (16, 16), columns: 5, rows: 1)))",
    )
    .expect("the atlas");
}

fn skin(frame: u32) -> Sprite {
    Sprite {
        texture: Some(AssetPath::new("skin.png").expect("a path")),
        atlas: Some(AssetPath::new("skin.atlas.ron").expect("a path")),
        frame,
        ..Sprite::default()
    }
}

fn named(world: &mut World, name: &str) -> Entity {
    let e = world.spawn();
    world.insert(e, SceneId::new());
    world.insert(e, Name::new(name));
    e
}

fn label(world: &mut World, parent: Entity, words: &str, size: f32) -> Entity {
    let e = named(world, &format!("{words} label"));
    world.insert(e, UiRect::stretched(4.0));
    world.insert(
        e,
        Text {
            size,
            color: [1.0, 1.0, 1.0, 1.0],
            align: Align::Center,
            ..Text::new(words)
        },
    );
    hierarchy::set_parent(world, e, parent).expect("parents");
    e
}

/// The menu, as an editor would author it: the panel centred on the
/// screen, a title and three buttons stacked down it.
///
/// The panel is 160 × 180 at (20, 10); 12 points of padding leave a column
/// 136 wide from y = 22 — the title 24 tall, then buttons 36 tall, 8 apart:
/// 54..90, 98..134 and 142..178.
fn pause_menu() -> World {
    let mut world = World::new();
    let panel = named(&mut world, "Pause menu");
    world.insert(
        panel,
        UiRect::pinned(Vec2::splat(0.5), Vec2::ZERO, Vec2::new(160.0, 180.0)),
    );
    world.insert(panel, skin(PANEL_FRAME));
    world.insert(panel, UiSlice::default());
    world.insert(
        panel,
        UiStack {
            direction: Direction::Column,
            spacing: 8.0,
            padding: Edges::uniform(12.0),
            justify: StackAlign::Start,
            align: StackAlign::Stretch,
        },
    );

    // Spawned out of their order on purpose: the slots decide it.
    for (slot, words) in [(3, "Quit"), (1, "Resume"), (2, "Options")] {
        let button = named(&mut world, words);
        world.insert(
            button,
            UiRect::pinned(Vec2::ZERO, Vec2::ZERO, Vec2::new(0.0, 36.0)).with_slot(slot),
        );
        world.insert(button, skin(0));
        world.insert(button, UiSlice::default());
        world.insert(
            button,
            UiButton {
                normal: [1.0; 4],
                hovered: [1.0; 4],
                pressed: [1.0; 4],
                disabled_color: [1.0; 4],
                disabled: false,
                frames: Some(ButtonFrames::in_a_row(0)),
            },
        );
        hierarchy::set_parent(&mut world, button, panel).expect("parents");
        label(&mut world, button, words, 14.0);
    }

    let title = named(&mut world, "Title");
    world.insert(
        title,
        UiRect::pinned(Vec2::ZERO, Vec2::ZERO, Vec2::new(0.0, 24.0)),
    );
    hierarchy::set_parent(&mut world, title, panel).expect("parents");
    label(&mut world, title, "Paused", 18.0);
    world
}

/// The menu written to a scene file and read back: what a game loads.
fn loaded(root: &Path) -> World {
    let registry = ComponentRegistry::with_defaults();
    let path = root.join("pause.ron");
    save(&pause_menu(), &registry, &path).expect("the menu saves");
    let mut world = World::new();
    load(&path, &registry, &mut world).expect("and loads");
    world
}

fn find(world: &World, name: &str) -> Entity {
    world
        .query::<Name>()
        .find(|(_, n)| n.as_str() == name)
        .map(|(e, _)| e)
        .unwrap_or_else(|| panic!("{name} is in the menu"))
}

fn frame(world: &mut World, buttons: &mut UiControls, pointer: Pointer, navigation: Navigation) {
    let layout = UiLayout::from_world(world, SCREEN);
    let input = UiInput {
        pointer,
        navigation,
        ..UiInput::default()
    };
    buttons.update(world, &layout, None, &input);
}

fn over(x: f32, y: f32) -> Pointer {
    Pointer {
        position: Some(Vec2::new(x, y)),
        ..Pointer::default()
    }
}

#[test]
fn the_menu_stacks_in_slot_order_inside_the_padded_panel() {
    let root = scratch_root();
    let world = loaded(&root);
    let layout = UiLayout::from_world(&world, SCREEN);
    let top = |name: &str| layout.rect(find(&world, name)).map(|r| (r.min, r.max));

    assert_eq!(
        top("Pause menu"),
        Some((Vec2::new(20.0, 10.0), Vec2::new(180.0, 190.0)))
    );
    assert_eq!(
        top("Title"),
        Some((Vec2::new(32.0, 22.0), Vec2::new(168.0, 46.0)))
    );
    assert_eq!(
        top("Resume"),
        Some((Vec2::new(32.0, 54.0), Vec2::new(168.0, 90.0)))
    );
    assert_eq!(
        top("Options"),
        Some((Vec2::new(32.0, 98.0), Vec2::new(168.0, 134.0)))
    );
    assert_eq!(
        top("Quit"),
        Some((Vec2::new(32.0, 142.0), Vec2::new(168.0, 178.0)))
    );
}

#[test]
fn a_click_on_a_label_clicks_its_button_and_the_keys_walk_the_column() {
    let root = scratch_root();
    let mut world = loaded(&root);
    let mut buttons = UiControls::default();
    let (resume, options, quit) = (
        find(&world, "Resume"),
        find(&world, "Options"),
        find(&world, "Quit"),
    );
    let shown = |world: &World, e: Entity| world.get::<Sprite>(e).expect("a face").frame;

    // Over the middle of Quit — its label — then down and up.
    frame(
        &mut world,
        &mut buttons,
        over(100.0, 160.0),
        Navigation::default(),
    );
    assert_eq!(shown(&world, quit), 1, "hovered");
    let down = Pointer {
        went_down: true,
        ..over(100.0, 160.0)
    };
    frame(&mut world, &mut buttons, down, Navigation::default());
    assert_eq!(shown(&world, quit), 2, "pressed");
    let up = Pointer {
        went_up: true,
        ..over(100.0, 160.0)
    };
    frame(&mut world, &mut buttons, up, Navigation::default());
    assert!(buttons.clicked(quit));
    assert!(!buttons.clicked(resume) && !buttons.clicked(options));

    // From the focus a click left on Quit, up is Options, then Resume.
    let up_key = Navigation {
        direction: Some(Vec2::new(0.0, -1.0)),
        accept: false,
        cancel: false,
    };
    frame(&mut world, &mut buttons, Pointer::default(), up_key);
    assert_eq!(buttons.focused(), Some(options));
    frame(&mut world, &mut buttons, Pointer::default(), up_key);
    assert_eq!(buttons.focused(), Some(resume));
    let accept = Navigation {
        direction: None,
        accept: true,
        cancel: false,
    };
    frame(&mut world, &mut buttons, Pointer::default(), accept);
    assert!(buttons.clicked(resume));
    assert_eq!(shown(&world, resume), 1, "the focus shows as hovered");
    assert_eq!(shown(&world, quit), 0, "and the rest as normal");
}

#[test]
fn the_menu_draws_sliced_and_shows_each_buttons_state() {
    let Some((device, queue)) = headless_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let root = scratch_root();
    write_skin(&root);
    copy_bundled_font(&root, DEFAULT_FONT);
    let mut world = loaded(&root);

    let layout = texture::bind_group_layout(&device);
    let mut textures = Textures::new(&device, &queue, &layout, &root);
    let mut atlases = Atlases::new(&root);
    let mut fonts = Fonts::new(&root);
    let faces: Vec<Entity> = world.query::<Sprite>().map(|(e, _)| e).collect();
    for e in faces {
        let sprite = world.get_mut::<Sprite>(e).expect("a sprite");
        let (texture, atlas) = (sprite.texture.clone(), sprite.atlas.clone());
        sprite.set_texture(texture, &mut textures, &device, &queue);
        sprite.set_atlas(atlas, &mut atlases);
    }
    resolve_fonts(&mut world, &mut fonts);
    prepare(&world, &mut fonts, 1.0);
    fonts.upload(&mut textures, &device, &queue);

    // Quit held down; the other two left alone.
    let mut buttons = UiControls::default();
    frame(
        &mut world,
        &mut buttons,
        over(100.0, 160.0),
        Navigation::default(),
    );
    let down = Pointer {
        went_down: true,
        ..over(100.0, 160.0)
    };
    frame(&mut world, &mut buttons, down, Navigation::default());
    let quit = find(&world, "Quit");
    assert_eq!(world.get::<Sprite>(quit).map(|s| s.frame), Some(2));

    let ui_layout = UiLayout::from_world(&world, SCREEN);
    let sheets = Sheets::new(&atlases, &textures).with_fonts(&fonts);
    let batch = ui::mesh::batch(&world, sheets, &ui_layout, 1.0);
    // Four sliced faces of nine pieces, and the letters of four labels.
    assert!(
        batch.sprite_count() > 4 * 9 + 20,
        "{}",
        batch.sprite_count()
    );

    let pixels = render(&device, &queue, &batch, &textures, &layout);
    let at = |x: u32, y: u32| pixels[(y * SIZE + x) as usize];
    let is = |p: Rgba, rgba: [u8; 4]| {
        (0..3).all(|i| (i32::from([p.r, p.g, p.b][i]) - i32::from(rgba[i])).abs() < 40)
    };

    // The panel: a yellow frame 4 points thick, then its magenta middle
    // — which, stretched whole, would still be frame 40 points in. Read a
    // little way into the middle: a linear sampler blends the frame's last
    // texel into the first few pixels of a stretched piece.
    assert!(is(at(21, 100), YELLOW), "panel frame: {:?}", at(21, 100));
    assert!(is(at(40, 50), MIDDLES[4]), "panel middle: {:?}", at(40, 50));
    assert!(is(at(100, 50), MIDDLES[4]), "between title and Resume");
    // Each button: a white frame 4 thick, then the middle of its state —
    // 34 points of frame, stretched whole.
    for (y, state, which) in [(72, 0, "Resume"), (116, 0, "Options"), (160, 2, "Quit")] {
        assert!(is(at(33, y), WHITE), "{which} frame: {:?}", at(33, y));
        assert!(
            is(at(50, y), MIDDLES[state]),
            "{which} middle, state {state}: {:?}",
            at(50, y)
        );
    }
    // The title's letters, white over the panel's green-less middle.
    let lit = (22..46)
        .flat_map(|y| (32..168).map(move |x| (x, y)))
        .filter(|&(x, y)| at(x, y).g > 150)
        .count();
    assert!(lit > 20, "the title is drawn: {lit} white pixels");
    // Outside the panel, the clear colour.
    assert!(!is(at(5, 5), YELLOW) && !is(at(5, 5), MIDDLES[4]));
}

/// Draws the screen-space batch the way `App` draws its UI pass.
fn render(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    batch: &SpriteBatch,
    textures: &Textures,
    layout: &wgpu::BindGroupLayout,
) -> Vec<Rgba> {
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("headless-menu-target"),
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let camera = CameraBinding::new(device);
    camera.upload(queue, &View2D::new(Vec2::ZERO, 1.0, 1.0));
    camera.upload_matrix(queue, screen_projection(SIZE as f32, SIZE as f32));
    let white = Texture::white(device, queue).create_bind_group(device, layout);
    let render_pipeline = pipeline::create_flat_color(device, FORMAT, camera.layout(), layout);

    let mesh = batch.upload(device);
    let draws: Vec<MeshDraw> = batch
        .ranges
        .iter()
        .map(|range| MeshDraw {
            texture: match range.texture {
                Some(handle) => textures.bind_group(handle),
                None => &white,
            },
            normal: None,
            light_layers: voltra_render::DEFAULT_LIGHT_LAYERS,
            primitives: range.primitives.clone(),
            material: None,
        })
        .collect();
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("headless-menu-encoder"),
    });
    pass::draw_mesh_batches(
        &mut encoder,
        &view,
        &render_pipeline,
        camera.bind_group(),
        mesh.as_ref(),
        &draws,
        CLEAR,
    );
    queue.submit(Some(encoder.finish()));
    read_texture(device, queue, &target, SIZE, SIZE)
}
