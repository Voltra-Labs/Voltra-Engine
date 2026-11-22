//! The `editor_*` tools against a stand-in editor: a bridge server on a
//! thread answering as the editor's frame would.

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::{call, client_of, repo_project, text_of};
use serde_json::json;
use voltra_bridge::{BridgeServer, Command};
use voltra_mcp::VoltraMcp;

/// A bridge that records each command and answers it; returns its port and
/// what it saw.
fn stand_in_editor() -> (u16, Arc<Mutex<Vec<Command>>>) {
    let server = BridgeServer::start(0).expect("a free port");
    let port = server.address().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    // A recorder that gains ten frames each time it is asked while on.
    let (mut recording, mut recorded) = (false, 0u64);
    std::thread::spawn(move || loop {
        for pending in server.poll() {
            let answer = match &pending.command {
                Command::Profile { record, .. } => {
                    if let Some(on) = record {
                        recording = *on;
                    }
                    if recording {
                        recorded += 10;
                    }
                    Ok(
                        json!({ "recording": recording, "frames_recorded": recorded, "frames": recorded.min(30) }),
                    )
                }
                Command::Spawn { name, .. } => Ok(json!({ "id": "abc", "name": name })),
                Command::Screenshot => {
                    Ok(json!({ "width": 2, "height": 1, "png_base64": "iVBORw0KGgo=" }))
                }
                Command::Save { .. } => Err("only while editing: stop play first".to_owned()),
                _ => Ok(json!({ "ok": true })),
            };
            log.lock()
                .expect("not poisoned")
                .push(pending.command.clone());
            pending.answer(answer);
        }
        std::thread::sleep(Duration::from_millis(5));
    });
    (port, seen)
}

#[tokio::test]
async fn tools_reach_the_editor_and_carry_its_answers_back() {
    let (port, seen) = stand_in_editor();
    let client = client_of(VoltraMcp::with_editor_port(repo_project(), port)).await;

    let spawned = call(
        &client,
        "editor_spawn",
        json!({ "name": "Box", "components": { "Sprite": "()" } }),
    )
    .await;
    assert_ne!(spawned.is_error, Some(true), "{}", text_of(&spawned));
    assert!(text_of(&spawned).contains("abc"));

    let stepped = call(
        &client,
        "editor_play",
        json!({ "action": "step", "frames": 3 }),
    )
    .await;
    assert_ne!(stepped.is_error, Some(true), "{}", text_of(&stepped));
    let undone = call(&client, "editor_history", json!({ "action": "undo" })).await;
    assert_ne!(undone.is_error, Some(true));

    let refused = call(&client, "editor_save", json!({})).await;
    assert_eq!(refused.is_error, Some(true));
    assert!(text_of(&refused).contains("only while editing"));

    let opened = call(
        &client,
        "editor_open",
        json!({ "path": "scenes/sandbox.ron", "discard_changes": true }),
    )
    .await;
    assert_ne!(opened.is_error, Some(true), "{}", text_of(&opened));

    let shot = call(&client, "editor_screenshot", json!({})).await;
    assert_ne!(shot.is_error, Some(true), "{}", text_of(&shot));
    assert!(
        shot.content.iter().any(|c| c.as_image().is_some()),
        "an image block"
    );
    assert!(
        !text_of(&shot).contains("png_base64"),
        "the image is not repeated as text"
    );

    let seen = seen.lock().expect("not poisoned");
    assert!(matches!(seen[0], Command::Spawn { .. }));
    assert!(matches!(seen[1], Command::Step { frames: 3 }));
    assert!(matches!(seen[2], Command::Undo));
    assert!(matches!(seen[3], Command::Save { path: None }));
    assert!(matches!(
        &seen[4],
        Command::Open { path, discard_changes: true } if path == "scenes/sandbox.ron"
    ));
}

#[tokio::test]
async fn no_editor_is_an_answer_naming_the_flag() {
    // A port nothing listens on: bound and dropped.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .expect("a free port")
        .port();
    let client = client_of(VoltraMcp::with_editor_port(repo_project(), port)).await;
    let status = call(&client, "editor_status", json!({})).await;
    assert_eq!(status.is_error, Some(true));
    assert!(
        text_of(&status).contains("--bridge"),
        "{}",
        text_of(&status)
    );
}

#[tokio::test]
async fn a_profile_records_until_it_has_the_frames_and_leaves_recording_off() {
    let (port, seen) = stand_in_editor();
    let client = client_of(VoltraMcp::with_editor_port(repo_project(), port)).await;
    let profile = call(&client, "editor_profile", json!({ "frames": 30 })).await;
    assert_ne!(profile.is_error, Some(true), "{}", text_of(&profile));
    assert!(
        text_of(&profile).contains("\"frames\": 30"),
        "{}",
        text_of(&profile)
    );

    let seen = seen.lock().expect("not poisoned");
    assert!(
        matches!(seen.first(), Some(Command::Profile { record: None, .. })),
        "asked first"
    );
    assert!(seen.iter().any(|c| matches!(
        c,
        Command::Profile {
            record: Some(true),
            ..
        }
    )));
    assert!(
        matches!(
            seen.last(),
            Some(Command::Profile {
                record: Some(false),
                ..
            })
        ),
        "off again, as it was found"
    );
}
