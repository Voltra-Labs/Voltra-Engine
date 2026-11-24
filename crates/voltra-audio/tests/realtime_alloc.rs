//! The device callback's work allocates nothing.
//!
//! A counting allocator around `Mixer::render` with every kind of work the
//! callback does in one mix — resident and streamed voices, a filter, a
//! delay, a reverb and a duck on buses, a parameter moving, buffers of a
//! length that is not a whole number of blocks. An allocation on the audio
//! thread is a call into a lock the game may hold, and so a gap in the sound;
//! this is the test that says there is none. One test in its own binary, so
//! nothing else allocates while it counts.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use voltra_audio::command::Command;
use voltra_audio::{
    probe, stream, Buses, Clip, Curve, Effect, EffectDesc, Mixer, Param, PlayParams, StreamSource,
    VoiceId,
};

struct Counting;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        // SAFETY: forwarded unchanged to the system allocator, whose contract
        // is the one this function has.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: as above; `pointer` came from `alloc` with this layout.
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        // SAFETY: as above.
        unsafe { System.realloc(pointer, layout, size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const RATE: u32 = 48_000;

/// A 16-bit mono WAV of a slow ramp, `seconds` long, in memory.
fn wav(seconds: u32) -> Vec<u8> {
    let frames = RATE * seconds;
    let mut file = Vec::new();
    file.extend_from_slice(b"RIFF");
    file.extend_from_slice(&(36 + frames * 2).to_le_bytes());
    file.extend_from_slice(b"WAVEfmt ");
    file.extend_from_slice(&16u32.to_le_bytes());
    file.extend_from_slice(&1u16.to_le_bytes());
    file.extend_from_slice(&1u16.to_le_bytes());
    file.extend_from_slice(&RATE.to_le_bytes());
    file.extend_from_slice(&(RATE * 2).to_le_bytes());
    file.extend_from_slice(&2u16.to_le_bytes());
    file.extend_from_slice(&16u16.to_le_bytes());
    file.extend_from_slice(b"data");
    file.extend_from_slice(&(frames * 2).to_le_bytes());
    for frame in 0..frames {
        file.extend_from_slice(&((frame % 2_000) as i16 * 4).to_le_bytes());
    }
    file
}

fn play(mixer: &mut Mixer, raw: u64, clip: Clip, feed: Option<stream::Feed>, bus: usize) {
    let mut buses = Buses::new(["music", "sfx", "dialogue"]);
    let bus = buses.id(["master", "music", "sfx", "dialogue"][bus]);
    mixer.apply(Command::Play {
        id: VoiceId::forge(raw),
        clip,
        feed,
        params: PlayParams {
            bus,
            looping: true,
            volume: 0.3,
            ..Default::default()
        },
        fade_in: Duration::from_millis(20),
        curve: Curve::EqualPower,
    });
}

#[test]
fn a_mix_with_every_kind_of_work_allocates_nothing() {
    let mut buses = Buses::new(["music", "sfx", "dialogue"]);
    let music = buses.id("music");
    let sfx = buses.id("sfx");
    let mut mixer = Mixer::new(RATE, 2);
    mixer.apply(Command::SetBusEffects {
        bus: music,
        effects: Effect::build_chain(
            &[
                EffectDesc::LowPass {
                    cutoff: 2_000.0,
                    q: 0.7,
                },
                EffectDesc::Duck {
                    by: "dialogue".to_owned(),
                    depth: 0.5,
                    threshold: 0.01,
                    attack: 0.05,
                    release: 0.2,
                },
            ],
            RATE,
            &mut buses,
        ),
    });
    mixer.apply(Command::SetBusEffects {
        bus: sfx,
        effects: Effect::build_chain(
            &[
                EffectDesc::HighPass {
                    cutoff: 200.0,
                    q: 0.7,
                },
                EffectDesc::Delay {
                    time: 0.3,
                    feedback: 0.4,
                    mix: 0.4,
                },
                EffectDesc::Reverb {
                    room_size: 0.7,
                    damping: 0.5,
                    width: 1.0,
                    mix: 0.3,
                },
            ],
            RATE,
            &mut buses,
        ),
    });

    let streamed = probe(StreamSource::Bytes {
        path: "long.wav".into(),
        bytes: Arc::from(wav(10)),
    })
    .expect("the WAV probes");
    let (mut decoder, feed) = stream::start(&streamed, true).expect("it opens");
    play(&mut mixer, 1, streamed.clone(), Some(feed), 1);
    play(
        &mut mixer,
        2,
        Clip::new(vec![0.2; 4_800], 1, 44_100),
        None,
        2,
    );
    play(&mut mixer, 3, Clip::new(vec![0.1; 9_600], 2, RATE), None, 3);

    // A length that is not a whole number of blocks, as a host may ask for.
    let mut out = vec![0.0f32; 1_000 * 2];
    mixer.render(&mut out);

    let control = ALLOCATIONS.load(Ordering::Relaxed);
    drop(std::hint::black_box(vec![0u8; 16]));
    assert!(
        ALLOCATIONS.load(Ordering::Relaxed) > control,
        "the counter counts"
    );

    let mut counted = 0;
    for round in 0..200 {
        decoder.fill();
        let param = Command::SetBusParam {
            bus: music,
            effect: 0,
            param: Param::Cutoff,
            value: 500.0 + round as f32 * 50.0,
            over: Duration::from_millis(30),
        };
        let before = ALLOCATIONS.load(Ordering::Relaxed);
        mixer.apply(param);
        mixer.render(&mut out);
        counted += ALLOCATIONS.load(Ordering::Relaxed) - before;
    }

    assert_eq!(mixer.voices(), 3, "everything was still playing");
    assert_eq!(counted, 0, "the mix allocated {counted} times");
}
