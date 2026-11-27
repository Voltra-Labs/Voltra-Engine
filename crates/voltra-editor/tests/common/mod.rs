//! What every test that launches the editor shares: a scratch project, the
//! editor process on it, and its bridge.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command as Process, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;
use voltra_bridge::{BridgeClient, Command};
use voltra_testkit::Scratch;

pub const WAIT: Duration = Duration::from_secs(30);

/// The scene every project starts on unless a test brings its own: one box.
pub const BOX: &str = r#"(version: 1, entities: [(id: "019fe500-0000-7000-8000-000000000001", components: {"Name": "Box", "Transform": (translation:(0.0,0.0),rotation:0.0,scale:(1.0,1.0)), "Sprite": (color:(1.0,0.5,0.2,1.0),sort_order:0)})])"#;

/// A project of its own: the engine's input map, layers and font, a scene
/// holding one box, and a `build.ron` beside the assets. Answers the
/// project's folder, removed when dropped, and its assets folder.
pub fn project(name: &str) -> (Scratch, PathBuf) {
    project_with(name, BOX)
}

/// [`project`] with `scene` as its `scenes/box.ron`.
pub fn project_with(name: &str, scene: &str) -> (Scratch, PathBuf) {
    let dir = Scratch::new(&format!("voltra-{name}-test"));
    let assets = dir.join("assets");
    let engine = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    for file in [
        "default.input.ron",
        "default.layers.ron",
        "fonts/Ubuntu-Light.ttf",
    ] {
        let to = assets.join(file);
        std::fs::create_dir_all(to.parent().expect("a folder")).expect("folders");
        std::fs::copy(engine.join(file), &to).expect("copied");
    }
    std::fs::create_dir_all(assets.join("scenes")).expect("scenes");
    std::fs::write(assets.join("scenes/box.ron"), scene).expect("the scene");
    std::fs::write(
        dir.join("build.ron"),
        r#"(version: 1, name: "Bridge Test", scene: "scenes/box.ron")"#,
    )
    .expect("the manifest");
    (dir, assets)
}

/// The editor process, killed when the test ends however it ends, and the
/// file its log goes to.
pub struct Editor {
    child: Child,
    log: PathBuf,
}

impl Editor {
    /// What the editor has logged so far, for a failure to show.
    pub fn log(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_else(|e| format!("(no log: {e})"))
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .expect("a free port")
}

/// Starts the editor on the project and waits for its bridge — failing at
/// once, with the editor's log, if it exits first, and with the log too if
/// it never answers.
pub fn launch(assets: &Path) -> (Editor, BridgeClient) {
    let port = free_port();
    let log = assets.with_file_name("editor.log");
    let stderr = std::fs::File::create(&log).expect("the log file");
    let child = Process::new(env!("CARGO_BIN_EXE_voltra-editor"))
        .args(["scenes/box.ron", "--bridge", &port.to_string()])
        .env("VOLTRA_ASSET_ROOT", assets)
        .env(
            "RUST_LOG",
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_owned()),
        )
        .stdout(Stdio::null())
        .stderr(stderr)
        .spawn()
        .expect("the editor starts");
    let mut editor = Editor { child, log };
    let start = Instant::now();
    loop {
        if let Ok(client) = BridgeClient::connect(port, Duration::from_secs(2)) {
            return (editor, client);
        }
        if let Ok(Some(status)) = editor.child.try_wait() {
            panic!(
                "the editor exited ({status}) before its bridge answered:\n{}",
                editor.log()
            );
        }
        assert!(
            start.elapsed() < Duration::from_secs(120),
            "the bridge never answered; the editor logged:\n{}",
            editor.log()
        );
        std::thread::sleep(Duration::from_millis(300));
    }
}

pub fn call(client: &mut BridgeClient, command: Command) -> Value {
    client
        .call(command, WAIT)
        .unwrap_or_else(|e| panic!("the bridge refused: {e}"))
}

/// Waits until the scene the editor was started on is open — its box can be
/// found — so what a test spawns is not swept away by the scene opening on
/// the editor's first frame.
pub fn wait_for_scene(client: &mut BridgeClient) {
    wait_for(client, "Box");
}

/// Waits until the scene is open: `entity` is there to be found.
pub fn wait_for(client: &mut BridgeClient, entity: &str) {
    let start = Instant::now();
    while client
        .call(
            Command::Rename {
                entity: entity.to_owned(),
                name: entity.to_owned(),
            },
            WAIT,
        )
        .is_err()
    {
        assert!(start.elapsed() < WAIT, "the scene never opened");
        std::thread::sleep(Duration::from_millis(200));
    }
}
