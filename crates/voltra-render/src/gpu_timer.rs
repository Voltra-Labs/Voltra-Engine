//! How long each pass takes on the GPU, read back frames later without
//! waiting for it.
//!
//! A timestamp is written into the encoder before and after a named pass
//! (`CommandEncoder::write_timestamp`, which needs `TIMESTAMP_QUERY` and
//! `TIMESTAMP_QUERY_INSIDE_ENCODERS`); when the encoder is finished the
//! timestamps are resolved and copied into one of a ring of readback
//! buffers, mapped after the submit and read whenever they have landed —
//! one to a few frames on, since the GPU runs behind the CPU. An adapter
//! without the features times nothing, and says so once.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

/// Timestamps one encoder may hold: two per scope.
const QUERIES: u32 = 128;
/// Encoders whose timings can be in flight at once: a few per frame, for
/// the frames the GPU is behind.
const READBACKS: usize = 8;

const FREE: u8 = 0;
const WAITING: u8 = 1;
const MAPPED: u8 = 2;
const FAILED: u8 = 3;

/// The features GPU timing needs, both of them.
pub fn timing_features() -> wgpu::Features {
    wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS
}

/// Times named passes on the GPU.
pub struct GpuTimer {
    queries: Option<Queries>,
    enabled: bool,
}

struct Queries {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readbacks: Vec<Readback>,
    /// The scopes of the encoder being recorded, by start query.
    open: Vec<&'static str>,
    /// Nanoseconds a timestamp tick is.
    period: f64,
    /// The readback the last `finish` filled, to map after the submit.
    filled: Option<usize>,
}

struct Readback {
    buffer: wgpu::Buffer,
    state: Arc<AtomicU8>,
    names: Vec<&'static str>,
}

/// A scope [`GpuTimer::begin`] opened.
#[derive(Debug, Clone, Copy)]
pub struct GpuScope(Option<u32>);

impl GpuTimer {
    /// A timer for `device`; one that times nothing when the device was not
    /// given [`timing_features`].
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let queries = device
            .features()
            .contains(timing_features())
            .then(|| Queries::new(device, queue));
        if queries.is_none() {
            log::info!("GPU timing unavailable: the adapter lacks timestamp queries in encoders");
        }
        Self {
            queries,
            enabled: false,
        }
    }

    /// Whether this device can time passes at all.
    pub fn is_supported(&self) -> bool {
        self.queries.is_some()
    }

    /// Times passes from now on, or stops.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Writes the start of `name` into `encoder`.
    pub fn begin(&mut self, encoder: &mut wgpu::CommandEncoder, name: &'static str) -> GpuScope {
        let Some(queries) = self.queries.as_mut().filter(|_| self.enabled) else {
            return GpuScope(None);
        };
        let index = queries.open.len() as u32 * 2;
        if index + 2 > QUERIES {
            return GpuScope(None);
        }
        queries.open.push(name);
        encoder.write_timestamp(&queries.set, index);
        GpuScope(Some(index))
    }

    /// Writes the end of `scope` into `encoder`.
    pub fn end(&mut self, encoder: &mut wgpu::CommandEncoder, scope: GpuScope) {
        if let (Some(queries), Some(index)) = (self.queries.as_ref(), scope.0) {
            encoder.write_timestamp(&queries.set, index + 1);
        }
    }

    /// Times `record`, the passes it puts into `encoder`, as `name`.
    pub fn time<R>(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        name: &'static str,
        record: impl FnOnce(&mut wgpu::CommandEncoder) -> R,
    ) -> R {
        let scope = self.begin(encoder, name);
        let out = record(encoder);
        self.end(encoder, scope);
        out
    }

    /// Resolves the encoder's timestamps into a free readback. Call once,
    /// last, before the encoder is finished; then [`submitted`] after the
    /// submit.
    ///
    /// [`submitted`]: Self::submitted
    pub fn finish(&mut self, encoder: &mut wgpu::CommandEncoder) {
        let Some(queries) = self.queries.as_mut() else {
            return;
        };
        let names = std::mem::take(&mut queries.open);
        if names.is_empty() {
            return;
        }
        let Some(slot) = queries
            .readbacks
            .iter()
            .position(|readback| readback.state.load(Ordering::Acquire) == FREE)
        else {
            // Every readback still waits on the GPU: this encoder's times are
            // dropped rather than the frame waited for.
            return;
        };
        let count = names.len() as u32 * 2;
        let bytes = u64::from(count) * u64::from(wgpu::QUERY_SIZE);
        encoder.resolve_query_set(&queries.set, 0..count, &queries.resolve, 0);
        let readback = &mut queries.readbacks[slot];
        encoder.copy_buffer_to_buffer(&queries.resolve, 0, &readback.buffer, 0, bytes);
        readback.names = names;
        readback.state.store(WAITING, Ordering::Release);
        queries.filled = Some(slot);
    }

    /// Maps the readback [`finish`](Self::finish) filled, now that its
    /// encoder is submitted.
    pub fn submitted(&mut self) {
        let Some(queries) = self.queries.as_mut() else {
            return;
        };
        let Some(slot) = queries.filled.take() else {
            return;
        };
        let readback = &queries.readbacks[slot];
        let state = Arc::clone(&readback.state);
        let bytes = readback.names.len() as u64 * 2 * u64::from(wgpu::QUERY_SIZE);
        readback
            .buffer
            .map_async(wgpu::MapMode::Read, ..bytes, move |result| {
                state.store(
                    if result.is_ok() { MAPPED } else { FAILED },
                    Ordering::Release,
                );
            });
    }

    /// Every pass timing that has landed since the last call, in
    /// milliseconds. Never waits for the GPU.
    pub fn collect(&mut self, device: &wgpu::Device) -> Vec<(&'static str, f64)> {
        let Some(queries) = self.queries.as_mut() else {
            return Vec::new();
        };
        if let Err(e) = device.poll(wgpu::PollType::Poll) {
            log::warn!("polling for GPU timings failed: {e}");
        }
        let mut out = Vec::new();
        for readback in &mut queries.readbacks {
            match readback.state.load(Ordering::Acquire) {
                MAPPED => {
                    let bytes = readback.names.len() as u64 * 2 * u64::from(wgpu::QUERY_SIZE);
                    if let Ok(view) = readback.buffer.get_mapped_range(..bytes) {
                        let (ticks, _) = view.as_chunks::<8>();
                        let (pairs, _) = ticks.as_chunks::<2>();
                        for (name, [start, end]) in readback.names.iter().zip(pairs) {
                            let start = u64::from_le_bytes(*start);
                            let end = u64::from_le_bytes(*end);
                            let ns = end.saturating_sub(start) as f64 * queries.period;
                            out.push((*name, ns / 1_000_000.0));
                        }
                    }
                    readback.buffer.unmap();
                    readback.state.store(FREE, Ordering::Release);
                }
                FAILED => {
                    log::warn!("a GPU timing readback would not map");
                    readback.state.store(FREE, Ordering::Release);
                }
                _ => {}
            }
        }
        out
    }
}

impl Queries {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let bytes = u64::from(QUERIES) * u64::from(wgpu::QUERY_SIZE);
        Self {
            set: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("gpu-timer"),
                ty: wgpu::QueryType::Timestamp,
                count: QUERIES,
            }),
            resolve: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gpu-timer-resolve"),
                size: bytes,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            readbacks: (0..READBACKS)
                .map(|_| Readback {
                    buffer: device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("gpu-timer-readback"),
                        size: bytes,
                        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                        mapped_at_creation: false,
                    }),
                    state: Arc::new(AtomicU8::new(FREE)),
                    names: Vec::new(),
                })
                .collect(),
            open: Vec::new(),
            period: f64::from(queue.get_timestamp_period()),
            filled: None,
        }
    }
}
