//! The tool's side: a connection to a running editor.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

use serde_json::Value;

use crate::protocol::{Command, Request, Response};

/// A connection to an editor's [`BridgeServer`](crate::BridgeServer).
pub struct BridgeClient {
    writer: TcpStream,
    reader: BufReader<TcpStream>,
    next_id: u64,
}

impl BridgeClient {
    /// Connects to an editor on this machine's `port`, giving up after
    /// `timeout`. `Err` when no editor is listening — the caller's cue to
    /// work on files instead.
    pub fn connect(port: u16, timeout: Duration) -> std::io::Result<Self> {
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        let stream = TcpStream::connect_timeout(&address, timeout)?;
        stream.set_nodelay(true)?;
        let reader = BufReader::new(stream.try_clone()?);
        Ok(Self {
            writer: stream,
            reader,
            next_id: 1,
        })
    }

    /// Sends `command` and waits for its answer, up to `timeout` — long
    /// enough for the editor to reach its next frame, or to step a while.
    pub fn call(&mut self, command: Command, timeout: Duration) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let line = serde_json::to_string(&Request { id, command }).map_err(|e| e.to_string())?;
        writeln!(self.writer, "{line}").map_err(|e| format!("the editor went away: {e}"))?;
        self.reader
            .get_ref()
            .set_read_timeout(Some(timeout))
            .map_err(|e| e.to_string())?;
        loop {
            let mut text = String::new();
            let read = self
                .reader
                .read_line(&mut text)
                .map_err(|e| format!("no answer from the editor: {e}"))?;
            if read == 0 {
                return Err("the editor closed the bridge".into());
            }
            let response: Response =
                serde_json::from_str(&text).map_err(|e| format!("a garbled answer: {e}"))?;
            // An answer to an earlier, timed-out request is skipped.
            if response.id == id {
                return response.into_result();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BridgeServer;

    #[test]
    fn a_request_goes_to_the_frame_and_its_answer_comes_back() {
        let server = BridgeServer::start(0).expect("a free port");
        let port = server.address().port();
        let editor = std::thread::spawn(move || {
            // The editor's frames: poll until something arrives, answer it.
            for _ in 0..500 {
                for pending in server.poll() {
                    let answer = match &pending.command {
                        Command::Status => Ok(serde_json::json!({ "scene": "demo" })),
                        _ => Err("not now".to_owned()),
                    };
                    pending.answer(answer);
                }
                std::thread::sleep(Duration::from_millis(2));
            }
        });
        let mut client = BridgeClient::connect(port, Duration::from_secs(2)).expect("connects");
        let status = client
            .call(Command::Status, Duration::from_secs(2))
            .expect("answered");
        assert_eq!(status["scene"], "demo");
        assert_eq!(
            client.call(Command::Undo, Duration::from_secs(2)),
            Err("not now".to_owned())
        );
        drop(client);
        let _ = editor.join();
    }

    #[test]
    fn no_editor_listening_is_an_error_to_fall_back_on() {
        // A port nothing listens on: bind one and drop it.
        let port = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .and_then(|l| l.local_addr())
            .map(|a| a.port())
            .expect("a port");
        assert!(BridgeClient::connect(port, Duration::from_millis(200)).is_err());
    }
}
