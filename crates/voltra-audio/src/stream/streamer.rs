//! What runs the [`Pump`]: a thread of its own where there are threads, the
//! main loop where there are not.

use std::sync::mpsc::Sender;

use super::decoder::StreamDecoder;
use super::pump::Pump;

/// How long the decoding thread sleeps between top-ups.
///
/// Five milliseconds against a second of buffer: a ring is never less than
/// nearly full, and the thread is asleep nearly all the time.
#[cfg(not(target_arch = "wasm32"))]
const PERIOD: std::time::Duration = std::time::Duration::from_millis(5);

/// Decodes every playing stream off the audio thread.
///
/// Natively the decoders are handed to a thread named `voltra-stream`,
/// started with the first stream and ended once the last decoder is done
/// and this is dropped. A browser build has no threads, and a native one
/// that could not start one falls back the same way: the decoders stay here
/// and [`pump`](Self::pump), called once a frame by
/// [`Audio::end_frame`](crate::Audio::end_frame), tops them up.
#[derive(Debug, Default)]
pub struct Streamer {
    thread: Option<Sender<StreamDecoder>>,
    /// Set once spawning failed, so it is not tried for every stream.
    no_thread: bool,
    local: Pump,
}

impl Streamer {
    /// A streamer with nothing to decode yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts decoding `decoder` wherever decoding runs.
    pub fn add(&mut self, decoder: StreamDecoder) {
        let decoder = match self.sender() {
            Some(sender) => match sender.send(decoder) {
                Ok(()) => return,
                // The thread is gone; decode here from now on.
                Err(returned) => {
                    self.thread = None;
                    self.no_thread = true;
                    returned.0
                }
            },
            None => decoder,
        };
        self.local.add(decoder);
    }

    /// Tops up the streams decoded on this thread: every stream on the web,
    /// none natively.
    pub fn pump(&mut self) {
        self.local.pump();
    }

    /// The decoding thread, started on first use where there can be one.
    fn sender(&mut self) -> Option<&Sender<StreamDecoder>> {
        if self.thread.is_none() && !self.no_thread {
            self.thread = spawn();
            self.no_thread = self.thread.is_none();
        }
        self.thread.as_ref()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn() -> Option<Sender<StreamDecoder>> {
    let (sender, receiver) = std::sync::mpsc::channel::<StreamDecoder>();
    let spawned = std::thread::Builder::new()
        .name("voltra-stream".to_owned())
        .spawn(move || {
            let mut pump = Pump::new();
            loop {
                if pump.is_empty() {
                    // Nothing to decode: sleep until something is, or end
                    // once the `Streamer` has gone.
                    match receiver.recv() {
                        Ok(decoder) => pump.add(decoder),
                        Err(_) => return,
                    }
                }
                for decoder in receiver.try_iter() {
                    pump.add(decoder);
                }
                pump.pump();
                std::thread::sleep(PERIOD);
            }
        });
    match spawned {
        Ok(_) => Some(sender),
        Err(e) => {
            log::warn!("no thread for streaming audio ({e}); decoding on the main loop instead");
            None
        }
    }
}

/// A browser has no threads to start: every stream is pumped by the loop.
#[cfg(target_arch = "wasm32")]
fn spawn() -> Option<Sender<StreamDecoder>> {
    None
}
