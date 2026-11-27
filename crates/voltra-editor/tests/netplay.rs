//! Network play driven through the bridge: two in-editor clients played
//! with 100 ms of latency, both joined and replicating, and the profiler's
//! net counters read back.
//!
//! Launches the editor, so it needs a display and a GPU adapter; ignored by
//! default. Under Xvfb with a software Vulkan driver:
//!
//!   xvfb-run -a cargo test -p voltra-editor --test netplay -- --ignored

use std::path::{Path, PathBuf};
use std::process::{Child, Command as Process, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;
use voltra_bridge::{BridgeClient, Command};

const WAIT: Duration = Duration::from_secs(30);

/// A project of its own: the engine's input map and layers, and a scene
/// holding one replicated box.
fn project() -> (voltra_testkit::Scratch, PathBuf) {
    let dir = voltra_testkit::Scratch::new("voltra-netplay-test");
    let assets = dir.join("assets");
    let engine = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    for file in ["default.input.ron", "default.layers.ron"] {
        let to = assets.join(file);
        std::fs::create_dir_all(to.parent().expect("a folder")).expect("folders");
        std::fs::copy(engine.join(file), &to).expect("copied");
    }
    std::fs::create_dir_all(assets.join("scenes")).expect("scenes");
    std::fs::write(
        assets.join("scenes/box.ron"),
        r#"(version: 1, entities: [(id: "019fe500-0000-7000-8000-0000000000b1", components: {"Name": "Box", "Transform": (translation:(1.0,2.0),rotation:0.0,scale:(1.0,1.0)), "Sprite": (color:(1.0,0.5,0.2,1.0),sort_order:0), "Replicated": (priority: 1.0)})])"#,
    )
    .expect("the scene");
    std::fs::write(
        dir.join("build.ron"),
        r#"(version: 1, name: "Netplay Test", scene: "scenes/box.ron")"#,
    )
    .expect("the manifest");
    (dir, assets)
}

/// The editor process, killed when the test ends however it ends.
struct Editor {
    child: Child,
    log: PathBuf,
}

impl Editor {
    fn log(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_else(|e| format!("(no log: {e})"))
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn launch(assets: &Path) -> (Editor, BridgeClient) {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .expect("a free port");
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
            panic!("the editor exited ({status}):\n{}", editor.log());
        }
        assert!(
            start.elapsed() < Duration::from_secs(120),
            "the bridge never answered; the editor logged:\n{}",
            editor.log()
        );
        std::thread::sleep(Duration::from_millis(300));
    }
}

fn call(editor: &Editor, client: &mut BridgeClient, command: Command) -> Value {
    client
        .call(command, WAIT)
        .unwrap_or_else(|e| panic!("the bridge refused: {e}\n{}", editor.log()))
}

fn net_status(editor: &Editor, client: &mut BridgeClient) -> Value {
    call(
        editor,
        client,
        Command::NetPlay {
            clients: None,
            latency_ms: None,
            jitter_ms: None,
            loss: None,
            bandwidth: None,
        },
    )
}

fn counter(profile: &Value, name: &str) -> Option<Value> {
    profile["counters"]
        .as_array()?
        .iter()
        .find(|c| c["name"] == name)
        .cloned()
}

#[test]
#[ignore = "launches the editor: needs a display and a GPU adapter"]
fn two_clients_played_with_100_ms_of_latency_and_the_net_counters_read_back() {
    let (_project, assets) = project();
    let (editor, mut client) = launch(&assets);
    let start = Instant::now();
    while call(&editor, &mut client, Command::Status)["entities"] != 1 {
        assert!(start.elapsed() < WAIT, "the scene never opened");
        std::thread::sleep(Duration::from_millis(200));
    }

    let set = call(
        &editor,
        &mut client,
        Command::NetPlay {
            clients: Some(2),
            latency_ms: Some(100.0),
            jitter_ms: None,
            loss: None,
            bandwidth: None,
        },
    );
    assert_eq!(set["clients_on_play"], 2);
    assert_eq!(set["conditions"]["latency_ms"], 100.0);
    // Out of range is refused, not clamped.
    assert!(client
        .call(
            Command::NetPlay {
                clients: Some(9),
                latency_ms: None,
                jitter_ms: None,
                loss: None,
                bandwidth: None,
            },
            WAIT,
        )
        .is_err());

    call(
        &editor,
        &mut client,
        Command::Profile {
            frames: 1,
            record: Some(true),
        },
    );
    call(&editor, &mut client, Command::Play);

    // Both clients in, with the box replicated to each.
    let start = Instant::now();
    let status = loop {
        let status = net_status(&editor, &mut client);
        let clients = status["clients"].as_array().cloned().unwrap_or_default();
        let joined = clients.len() == 2
            && clients.iter().all(|c| {
                !c["id"].is_null() && c["replicated"] == 1 && c["snapshots"].as_u64() > Some(30)
            });
        if joined {
            break status;
        }
        assert!(
            start.elapsed() < WAIT,
            "the clients never joined: {status}\n{}",
            editor.log()
        );
        std::thread::sleep(Duration::from_millis(200));
    };
    assert_eq!(status["running"], true);
    assert_eq!(status["server"].as_array().map(Vec::len), Some(2));
    // Two seconds of play, for the round trips to settle.
    std::thread::sleep(Duration::from_secs(2));
    let status = net_status(&editor, &mut client);
    for peer in status["server"].as_array().expect("the server's peers") {
        let rtt = peer["rtt_ms"].as_f64().expect("a round trip");
        assert!(
            (180.0..400.0).contains(&rtt),
            "100 ms each way is a 200 ms round trip, not {rtt}: {status}"
        );
    }

    let profile = call(
        &editor,
        &mut client,
        Command::Profile {
            frames: 60,
            record: None,
        },
    );
    let rtt =
        counter(&profile, "net rtt ms").unwrap_or_else(|| panic!("no rtt counter: {profile}"));
    assert!(
        (180.0..400.0).contains(&rtt["last"].as_f64().expect("a value")),
        "{rtt}"
    );
    let peers = counter(&profile, "net peers").expect("a peers counter");
    assert_eq!(peers["last"], 2.0, "the two clients, the host not a peer");
    for name in [
        "net bytes out",
        "net bytes in",
        "net packets out",
        "net packets in",
    ] {
        let value = counter(&profile, name).unwrap_or_else(|| panic!("no {name}: {profile}"));
        assert!(value["avg"].as_f64() > Some(0.0), "{name}: {value}");
    }

    call(&editor, &mut client, Command::Stop);
    let status = net_status(&editor, &mut client);
    assert_eq!(status["running"], false);
    assert_eq!(status["clients"].as_array().map(Vec::len), Some(0));
}
