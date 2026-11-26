mod atlas_doc;
mod bridge;
mod camera;
mod clipboard;
mod console;
mod delete;
mod document;
mod drag;
mod edit_keys;
mod editor;
mod game;
mod gizmo;
mod grab;
mod instance;
mod joints;
mod marquee;
mod netplay;
mod pack_atlas;
mod paint;
mod panels;
mod particle_preview;
mod picking;
mod play;
mod prefab;
mod prefab_mode;
mod project_settings;
mod scene_dialogs;
mod scene_file;
mod search;
mod selection;
mod shape_edit;
mod snap;
mod spawn;
mod timeline;
mod tool;
mod undo;
mod view;
mod weight_paint;

use editor::Editor;
use voltra_assets::AssetPath;
use voltra_core::{App, WindowConfig};
use voltra_render::glam::Vec2;
use voltra_scene::{SceneId, Sprite, Transform};

/// The sample texture this repository ships, under the asset root.
const CHECKER: &str = "sprites/checker.png";

fn main() {
    // The engine's logger, not a bare `env_logger`: it copies every line
    // into the ring the Console window and the bridge read.
    voltra_core::logging::init(false);

    let Some(args) = Args::parse(std::env::args().skip(1)) else {
        log::error!(
            "usage: voltra-editor [scene.ron] [--bridge [port]] [--game library-or-build.ron]"
        );
        std::process::exit(2);
    };

    let mut app = App::new(WindowConfig {
        title: "Voltra Editor".into(),
        ..Default::default()
    });

    let mut editor = Editor::default();
    // A game's own manifest brings its own assets: its scenes, its input
    // map, its sprites are under the folder it names.
    if let Some(manifest) = args
        .game
        .as_ref()
        .filter(|p| p.extension().is_some_and(|e| e == "ron"))
    {
        match game::project_assets(manifest) {
            Ok(root) => app = app.with_asset_root(root),
            Err(problem) => log::error!("{problem}"),
        }
    }
    // A library named on the command line; otherwise the project's, from
    // `build.ron`, loaded on the first frame either way.
    if let Some(path) = args.game {
        editor.game = game::GameHost::at(path);
    }
    match args.scene {
        // Opened on the first frame: opening needs the frame's stores.
        Some(scene) => editor.dialogs.open_at_start(scene),
        // Something to look at on the first run, before anything has been
        // opened. It names the checker this repository ships, so the texture
        // path — and hot reload over it — is visible without opening a scene.
        None => spawn_demo_scene(&mut app),
    }
    // `--bridge [port]`: let `voltra-mcp` drive this editor, on this
    // machine only. Off unless asked for.
    if let Some(port) = args.bridge {
        match voltra_bridge::BridgeServer::start(port) {
            Ok(server) => editor.bridge = Some(server),
            Err(e) => log::error!("could not start the editor bridge on port {port}: {e}"),
        }
    }
    app.with_ui(move |ui, frame| editor.ui(ui, frame))
        .with_hot_reload()
        // The close button asks "save changes?" when there are some.
        .with_close_guard()
        // No `with_simulation`: the editor starts in `Editing`, which is the
        // correct default for an authoring tool, and the toolbar's Play is what
        // turns the switch on.
        .run();
}

/// What the command line asked for.
#[derive(Debug, Default, PartialEq)]
struct Args {
    /// A scene to open, as an asset path.
    scene: Option<AssetPath>,
    /// The port `--bridge` asks for — its argument, or the default.
    bridge: Option<u16>,
    /// The game `--game` names — a library, or a build manifest naming its
    /// crate — instead of the project's.
    game: Option<std::path::PathBuf>,
}

impl Args {
    /// `None` for something it does not understand: an unknown flag, a
    /// second scene, a scene that is not a scene path.
    fn parse(args: impl Iterator<Item = String>) -> Option<Self> {
        let mut parsed = Self::default();
        let mut args = args.peekable();
        while let Some(arg) = args.next() {
            if arg == "--bridge" {
                let port = args
                    .next_if(|next| next.parse::<u16>().is_ok())
                    .and_then(|port| port.parse().ok());
                parsed.bridge = Some(port.unwrap_or(voltra_bridge::DEFAULT_PORT));
            } else if arg == "--game" {
                parsed.game = Some(args.next()?.into());
            } else if arg.starts_with("--") || parsed.scene.is_some() {
                return None;
            } else {
                match scene_file::scene_path(&arg) {
                    Ok(path) => parsed.scene = Some(path),
                    Err(e) => {
                        log::error!("{e}");
                        return None;
                    }
                }
            }
        }
        Some(parsed)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(line: &str) -> Option<Args> {
        Args::parse(line.split_whitespace().map(str::to_owned))
    }

    #[test]
    fn a_scene_and_the_bridge_come_in_either_order() {
        let both = parse("scenes/a.ron --bridge 47900").expect("parses");
        assert_eq!(
            both.scene.as_ref().map(AssetPath::as_str),
            Some("scenes/a.ron")
        );
        assert_eq!(both.bridge, Some(47900));
        let flag_first = parse("--bridge scenes/a.ron").expect("parses");
        assert_eq!(flag_first.bridge, Some(voltra_bridge::DEFAULT_PORT));
        assert!(flag_first.scene.is_some());
        assert_eq!(parse(""), Some(Args::default()));
        assert_eq!(parse("--nope"), None);
        assert_eq!(parse("a.ron b.ron"), None);
        assert_eq!(parse("a.png"), None);
        let game = parse("--game target/debug/libgame.so scenes/a.ron").expect("parses");
        assert_eq!(game.game, Some("target/debug/libgame.so".into()));
        assert!(game.scene.is_some());
        assert_eq!(parse("--game"), None, "the flag needs its path");
    }
}
