//! A long file streams with its memory bounded by the ring, not its length.
//!
//! The one claim streaming makes, measured rather than argued: a counting
//! allocator records the most heap the process held while a three-minute
//! file played through the mixer from start to end, and that has to be a
//! small fraction of what the file would be resident. One test in its own
//! binary, so no other test's allocations land in the count.

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use voltra_audio::command::Command;
use voltra_audio::{probe, stream, Curve, Mixer, PlayParams, StreamSource, VoiceId};

/// The system allocator, counting the bytes it has out and the most it ever
/// had.
struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarded unchanged to the system allocator, whose contract
        // is the one this function has.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(live, Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: as above; `pointer` came from `alloc` with this layout.
        unsafe { System.dealloc(pointer, layout) };
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const RATE: u32 = 16_000;
const SECONDS: u32 = 180;
const CHANNELS: u16 = 2;

/// Writes a 16-bit stereo WAV of a sawtooth a block at a time, so the
/// fixture itself never holds the file in memory.
fn write_long_wav(path: &Path) -> u64 {
    let frames = u64::from(RATE * SECONDS);
    let data_len = frames * u64::from(CHANNELS) * 2;
    let file = std::fs::File::create(path).expect("the fixture opens");
    let mut out = BufWriter::new(file);
    let header = |out: &mut BufWriter<std::fs::File>| -> std::io::Result<()> {
        out.write_all(b"RIFF")?;
        out.write_all(&(36 + data_len as u32).to_le_bytes())?;
        out.write_all(b"WAVEfmt ")?;
        out.write_all(&16u32.to_le_bytes())?;
        out.write_all(&1u16.to_le_bytes())?;
        out.write_all(&CHANNELS.to_le_bytes())?;
        out.write_all(&RATE.to_le_bytes())?;
        out.write_all(&(RATE * u32::from(CHANNELS) * 2).to_le_bytes())?;
        out.write_all(&(CHANNELS * 2).to_le_bytes())?;
        out.write_all(&16u16.to_le_bytes())?;
        out.write_all(b"data")?;
        out.write_all(&(data_len as u32).to_le_bytes())
    };
    header(&mut out).expect("the header writes");
    for frame in 0..frames {
        // Never zero, so a gap in the output is an underrun and not the
        // signal.
        let sample = 100 + (frame % 1_000) as i16 * 20;
        out.write_all(&sample.to_le_bytes()).expect("writes");
        out.write_all(&(-sample).to_le_bytes()).expect("writes");
    }
    out.flush().expect("the fixture flushes");
    frames
}

#[test]
fn a_three_minute_file_streams_in_a_few_hundred_kilobytes() {
    let path = voltra_testkit::scratch_root().join("long.wav");
    let frames = write_long_wav(&path);
    let resident = frames as usize * usize::from(CHANNELS) * std::mem::size_of::<f32>();

    let clip = probe(StreamSource::File(path)).expect("the long file probes");
    assert_eq!(clip.frames() as u64, frames);
    let mut mixer = Mixer::new(RATE, 2);
    let mut out = vec![0.0f32; 512 * 2];

    let baseline = LIVE.load(Ordering::Relaxed);
    PEAK.store(baseline, Ordering::Relaxed);

    let (mut decoder, feed) = stream::start(&clip, false).expect("the stream opens");
    mixer.apply(Command::Play {
        id: VoiceId::forge(1),
        clip: clip.clone(),
        feed: Some(feed),
        params: PlayParams::default(),
        fade_in: Duration::ZERO,
        curve: Curve::Linear,
    });

    let mut rendered = 0u64;
    let mut silent_blocks = 0;
    while mixer.voices() > 0 {
        decoder.fill();
        mixer.render(&mut out);
        rendered += 512;
        if out.iter().all(|sample| *sample == 0.0) {
            silent_blocks += 1;
        }
        assert!(
            rendered < frames + u64::from(RATE),
            "the stream never ended"
        );
    }
    let peak = PEAK.load(Ordering::Relaxed) - baseline;

    assert!(
        rendered >= frames,
        "it ended early: {rendered} of {frames} frames"
    );
    assert!(
        silent_blocks <= 1,
        "{silent_blocks} silent blocks: the decoder fell behind"
    );
    assert!(
        peak < 2 * 1024 * 1024,
        "streaming held {peak} bytes at its peak; the file resident is {resident}"
    );
    assert!(
        peak * 20 < resident,
        "{peak} is not a small part of {resident}"
    );
}
