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
