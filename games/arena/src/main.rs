//! Arena as a build: its first scene, its game, a window.
//!
//! `cargo run -p arena` plays alone; `-- --host` hosts a match others join
//! (UDP 7777 for native players, WebSocket 7778 for browsers); `-- --join
//! udp://HOST:7777` joins one. In a browser the page's `?join=ws://HOST:7778`
//! joins. `--bot` lets a bot steer, for a test. `cargo xtask dist
//! --manifest games/arena/build.ron` packages it, and `--web` for a
//! browser.

// A packaged build is double-clicked: no console behind the game in
// release, and the log in `log.txt` beside it. Debug builds keep the
// terminal they were run from.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use arena::{replication_config, Arena, Pilot, Session, FIRST_SCENE};
use voltra_assets::AssetPath;
use voltra_core::{App, WindowConfig};

fn main() {
    voltra_core::logging::init(cfg!(not(debug_assertions)));
    // Natively at once; in a browser once the game's pack has arrived.
    voltra_core::when_assets_ready(start);
}

/// The session and pilot the command line — or the page's address — asks
/// for.
fn chosen() -> Result<(Session, Pilot), String> {
    let mut session = Session::Solo;
    let mut pilot = Pilot::Keys;
    #[cfg(target_arch = "wasm32")]
    if let Some(server) = page_join() {
        session = Session::Join(server);
    }
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--host" => session = Session::host(),
            "--join" => {
                session = Session::Join(args.next().ok_or("--join needs a server address")?);
            }
            "--bot" => pilot = Pilot::Bot,
            other => {
                return Err(format!(
                    "unknown option {other:?}: --host, --join ADDR, --bot"
                ))
            }
        }
    }
    Ok((session, pilot))
}

/// `?join=ws://…` from the page's address.
#[cfg(target_arch = "wasm32")]
fn page_join() -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    search
        .trim_start_matches('?')
        .split('&')
        .find_map(|pair| pair.strip_prefix("join="))
        .map(|value| {
            js_sys::decode_uri_component(value)
                .ok()
                .and_then(|v| v.as_string())
                .unwrap_or_else(|| value.to_owned())
        })
}

fn start() -> Result<(), String> {
    let (session, pilot) = chosen()?;
    let scene = AssetPath::new(FIRST_SCENE).map_err(|e| e.to_string())?;
    let mut app = App::new(WindowConfig {
        title: "Arena".into(),
        ..Default::default()
    })
    .with_game_name("Arena")
    .with_replication(replication_config())
    .with_game(Arena::new(session, pilot));
    // A top-down arena: nothing falls.
    app.gravity = voltra_render::glam::Vec2::ZERO;
    app.load_scene(&scene);
    app.with_simulation().run();
    Ok(())
}
