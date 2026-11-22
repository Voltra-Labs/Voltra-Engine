//! The editor's side: a listener that hands requests to the frame and
//! carries the answers back.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender};

use serde_json::Value;

use crate::protocol::{Command, Request, Response};

/// A request waiting for the editor's frame, and the way back to whoever
/// sent it.
pub struct Pending {
    pub id: u64,
    pub command: Command,
    reply: Sender<Response>,
}

impl Pending {
    /// Answers it. A client that has gone is no error: the editor carries on.
    pub fn answer(self, result: Result<Value, String>) {
        let _ = self.reply.send(Response::answer(self.id, result));
    }
}

/// Listens on the loopback interface and queues what clients send.
///
/// Each connection gets a thread that reads one request per line, queues it
/// for [`poll`](Self::poll), waits for its answer and writes it back — so a
/// client's requests are applied one at a time, in order, each in a frame.
pub struct BridgeServer {
    queue: Receiver<Pending>,
    address: SocketAddr,
}

impl BridgeServer {
    /// Listens on `127.0.0.1:port` — 0 picks a free port — and only there:
    /// nothing off this machine can reach it.
    pub fn start(port: u16) -> std::io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))?;
        let address = listener.local_addr()?;
        let (queued, queue) = mpsc::channel();
        std::thread::Builder::new()
            .name("voltra-bridge".into())
            .spawn(move || accept(listener, queued))?;
        log::info!("editor bridge listening on {address}");
        Ok(Self { queue, address })
    }

    /// Where it listens.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Every request that arrived since the last poll. Never blocks.
    pub fn poll(&self) -> Vec<Pending> {
        let mut pending = Vec::new();
        while let Ok(request) = self.queue.try_recv() {
            pending.push(request);
        }
        pending
    }
}

fn accept(listener: TcpListener, queued: Sender<Pending>) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else {
            continue;
        };
        // Bound to loopback, but a peer is checked all the same.
        if !stream.peer_addr().is_ok_and(|peer| peer.ip().is_loopback()) {
            continue;
        }
        let queued = queued.clone();
        let spawned = std::thread::Builder::new()
            .name("voltra-bridge-client".into())
            .spawn(move || serve(stream, queued));
        if let Err(e) = spawned {
            log::warn!("editor bridge: could not serve a client: {e}");
        }
    }
}

fn serve(stream: TcpStream, queued: Sender<Pending>) {
    let Ok(mut writer) = stream.try_clone() else {
        return;
    };
    for line in BufReader::new(stream).lines() {
        let Ok(line) = line else {
            return;
        };
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Request>(&line) {
            Ok(request) => {
                let (reply, answer) = mpsc::channel();
                let id = request.id;
                if queued
                    .send(Pending {
                        id,
                        command: request.command,
                        reply,
                    })
                    .is_err()
                {
                    return;
                }
                answer
                    .recv()
                    .unwrap_or_else(|_| Response::answer(id, Err("the editor closed".into())))
            }
            Err(e) => Response::answer(0, Err(format!("not a bridge request: {e}"))),
        };
        let Ok(text) = serde_json::to_string(&response) else {
            return;
        };
        if writeln!(writer, "{text}").is_err() {
            return;
        }
    }
}
