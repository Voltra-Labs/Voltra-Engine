//! Talking to a running editor through its bridge.

use std::time::Duration;

use serde_json::Value;
use voltra_bridge::{BridgeClient, Command};

use super::ToolError;

/// How long a connection attempt waits for an editor to answer.
const CONNECT: Duration = Duration::from_millis(500);
/// How long a command waits for its answer: a frame, or a few hundred
/// steps.
const ANSWER: Duration = Duration::from_secs(30);

/// Sends `command` to the editor listening on `port` and answers its reply.
/// Blocking: call it off the async runtime.
pub fn call(port: u16, command: Command) -> Result<Value, ToolError> {
    let mut client = BridgeClient::connect(port, CONNECT).map_err(|e| {
        ToolError(format!(
            "no editor is listening on port {port} ({e}): start it with `voltra-editor --bridge`"
        ))
    })?;
    client.call(command, ANSWER).map_err(ToolError)
}

/// How long `profile` waits for the editor to record what was asked.
const RECORDING: Duration = Duration::from_secs(15);

/// The running editor's last `frames` frames, summarised. Starts recording
/// when it is off, waits for the frames, and stops it again after — leaving
/// recording as it found it.
pub fn profile(port: u16, frames: u32) -> Result<Value, ToolError> {
    let frames = frames.clamp(1, 600);
    let first = call(
        port,
        Command::Profile {
            frames,
            record: None,
        },
    )?;
    let was_recording = first["recording"].as_bool().unwrap_or(false);
    if !was_recording {
        call(
            port,
            Command::Profile {
                frames,
                record: Some(true),
            },
        )?;
    }
    let deadline = std::time::Instant::now() + RECORDING;
    loop {
        let now = call(
            port,
            Command::Profile {
                frames,
                record: None,
            },
        )?;
        let enough = now["frames_recorded"].as_u64().unwrap_or(0) >= u64::from(frames);
        if enough || std::time::Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let record = (!was_recording).then_some(false);
    call(port, Command::Profile { frames, record })
}
