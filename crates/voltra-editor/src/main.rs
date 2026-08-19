mod camera;
mod drag;
mod editor;
mod gizmo;
mod panels;
mod picking;
mod play;
mod spawn;
mod tool;
mod undo;
mod view;

use editor::Editor;
use voltra_assets::AssetPath;
use voltra_core::{App, WindowConfig};
use voltra_render::glam::Vec2;
use voltra_scene::{SceneId, Sprite, Transform};

/// The sample texture this repository ships, under the asset root.
const CHECKER: &str = "sprites/checker.png";

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let mut app = App::new(WindowConfig {
        title: "Voltra Editor".into(),
        ..Default::default()
    });

    // Something to look at on the first run, before anything has been opened
    // from the menu. It names the checker this repository ships, so the texture
    // path — and hot reload over it — is visible without opening a scene first.
    spawn_demo_scene(&mut app);

    let mut editor = Editor::default();
    app.with_ui(move |ui, frame| editor.ui(ui, frame))
        .with_hot_reload()
        // No `with_physics`: the editor starts in `Editing`, which is the
        // correct default for an authoring tool, and the toolbar's Play is what
        // turns the switch on.
        .run();
}

fn spawn_demo_scene(app: &mut App) {
    let palette = [
        ([1.0, 0.35, 0.35, 1.0], Vec2::new(-0.6, 0.0), 0.0),
        ([0.35, 1.0, 0.45, 1.0], Vec2::new(0.0, 0.0), 0.4),
        ([0.4, 0.55, 1.0, 1.0], Vec2::new(0.6, 0.0), 0.8),
    ];

    for (color, position, rotation) in palette {
        let entity = app.world.spawn();
        app.world.insert(entity, SceneId::new());
        app.world.insert(
            entity,
            Transform::from_translation(position)
                .with_scale(Vec2::splat(0.4))
                .with_rotation(rotation),
        );
        let mut sprite = Sprite::new(color);
        // Only the path. The handle is `None` until `resumed` resolves it,
        // because there is no device to upload with until a window exists.
        sprite.texture = Some(AssetPath::new(CHECKER).expect("a valid asset path"));
        app.world.insert(entity, sprite);
    }
}
