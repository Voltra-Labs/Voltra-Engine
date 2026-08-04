use voltra_core::{App, WindowConfig};
use voltra_render::glam::Vec2;
use voltra_scene::{Sprite, Transform};

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let mut app = App::new(WindowConfig {
        title: "Voltra Editor".into(),
        ..Default::default()
    });

    // A placeholder scene until there is an editor UI to build one with.
    spawn_demo_scene(&mut app);

    app.run();
}

fn spawn_demo_scene(app: &mut App) {
    let palette = [
        ([1.0, 0.35, 0.35, 1.0], Vec2::new(-0.6, 0.0), 0.0),
        ([0.35, 1.0, 0.45, 1.0], Vec2::new(0.0, 0.0), 0.4),
        ([0.4, 0.55, 1.0, 1.0], Vec2::new(0.6, 0.0), 0.8),
    ];

    for (color, position, rotation) in palette {
        let entity = app.world.spawn();
        app.world.insert(
            entity,
            Transform::from_translation(position)
                .with_scale(Vec2::splat(0.4))
                .with_rotation(rotation),
        );
        app.world.insert(entity, Sprite::new(color));
    }
}
