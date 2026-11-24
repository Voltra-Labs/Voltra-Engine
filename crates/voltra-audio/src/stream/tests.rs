use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::*;
use crate::command::Command;
use crate::decode::probe;
use crate::mixer::Mixer;
use crate::params::PlayParams;
use crate::ramp::Curve;
use crate::stream::decoder::Progress;
use crate::testing::wav;
use crate::voice::VoiceId;

const RATE: u32 = 1_000;

/// The value frame `index` of [`ramp_clip`] holds, as the decoder reads it.
fn value(index: usize) -> f32 {
    (index as f32 * 10.0) / 32_768.0
}

/// A mono WAV in memory whose frame `i` is `value(i)`: every frame says
/// where it is, so a test can read the order back out of the mix.
fn ramp_clip(frames: usize) -> Clip {
    let samples: Vec<i16> = (0..frames).map(|i| (i * 10) as i16).collect();
    let source = StreamSource::Bytes {
        path: PathBuf::from("ramp.wav"),
        bytes: Arc::from(wav(&samples, 1, RATE)),
    };
    probe(source).expect("a hand-written WAV probes")
}

/// Hard left, so the left channel is the clip's own samples.
fn left(looping: bool) -> PlayParams {
    PlayParams {
        pan: -1.0,
        looping,
        ..Default::default()
    }
}

/// Plays `clip` through a mixer at its own rate for `frames` frames,
/// pumping the decoder between buffers the way the streamer would, and
/// returns the left channel.
fn stream_through(clip: &Clip, looping: bool, frames: usize) -> (Vec<f32>, Mixer) {
    let (mut decoder, feed) = start(clip, looping).expect("the stream opens");
    let mut mixer = Mixer::new(RATE, 2);
    mixer.apply(Command::Play {
        id: VoiceId::forge(1),
        clip: clip.clone(),
        feed: Some(feed),
        params: left(looping),
        fade_in: Duration::ZERO,
        curve: Curve::Linear,
    });
    let mut heard = Vec::new();
    let mut out = vec![0.0; 64 * 2];
    while heard.len() < frames {
        decoder.fill();
        mixer.render(&mut out);
        heard.extend(out.iter().step_by(2).copied());
    }
    heard.truncate(frames);
    (heard, mixer)
}

#[test]
fn a_streamed_clip_reports_the_shape_its_header_declares() {
    let clip = ramp_clip(300);
    assert!(clip.is_streamed());
    assert_eq!(clip.frames(), 300);
    assert_eq!(clip.rate(), RATE);
    assert_eq!(clip.channels(), 1);
}

#[test]
fn a_stream_plays_every_frame_in_order_and_then_ends() {
    let clip = ramp_clip(300);
    let (heard, mixer) = stream_through(&clip, false, 400);

    for (index, sample) in heard.iter().enumerate().take(300) {
        assert!(
            (sample - value(index)).abs() < 1e-5,
            "frame {index}: {sample} vs {}",
            value(index)
        );
    }
    assert!(heard[300..].iter().all(|s| *s == 0.0), "then silence");
    assert_eq!(mixer.voices(), 0, "and the voice is gone");
}

#[test]
fn a_looping_stream_plays_the_intro_once_then_the_loop() {
    let clip = ramp_clip(100).with_loop(40, Some(80));
    let (heard, mixer) = stream_through(&clip, true, 300);

    let expected: Vec<usize> = (0..80)
        .chain((0..).flat_map(|_| 40..80))
        .take(300)
        .collect();
    for (frame, (sample, index)) in heard.iter().zip(expected).enumerate() {
        assert!(
            (sample - value(index)).abs() < 1e-5,
            "output frame {frame} should be file frame {index}: got {sample}"
        );
    }
    assert_eq!(mixer.voices(), 1, "a loop never ends by itself");
}

#[test]
fn a_looping_stream_with_no_loop_end_turns_back_at_the_file_s_end() {
    let clip = ramp_clip(50).with_loop(10, None);
    let (heard, _) = stream_through(&clip, true, 130);

    let expected: Vec<usize> = (0..50)
        .chain((0..).flat_map(|_| 10..50))
        .take(130)
        .collect();
    for (sample, index) in heard.iter().zip(expected) {
        assert!((sample - value(index)).abs() < 1e-5);
    }
}

#[test]
fn a_ring_holds_one_buffer_s_worth_and_no_more() {
    let clip = ramp_clip(RATE as usize * 5);
    let (mut decoder, feed) = start(&clip, false).expect("the stream opens");

    decoder.fill();

    let second = (BUFFER.as_secs_f64() * f64::from(RATE)) as usize;
    assert_eq!(decoder.buffered_capacity(), second);
    assert_eq!(feed.queued(), second, "filled to the brim and no further");
}

#[test]
fn a_new_stream_is_primed_before_its_voice_starts() {
    let clip = ramp_clip(RATE as usize * 5);
    let (_decoder, feed) = start(&clip, false).expect("the stream opens");
    let primed = (PRIME.as_secs_f64() * f64::from(RATE)) as usize;
    assert!(feed.queued() >= primed, "only {} queued", feed.queued());
}

#[test]
fn a_dropped_feed_ends_its_decoder() {
    // A voice that was stopped: nothing should go on decoding for it.
    let clip = ramp_clip(RATE as usize * 5);
    let (mut decoder, feed) = start(&clip, true).expect("the stream opens");
    drop(feed);
    assert_eq!(decoder.fill(), Progress::Done);
}

#[test]
fn the_streamer_decodes_off_the_calling_thread() {
    let clip = ramp_clip(RATE as usize * 5);
    let (decoder, feed) = start(&clip, false).expect("the stream opens");
    let mut streamer = Streamer::new();

    streamer.add(decoder);

    let deadline = Instant::now() + Duration::from_secs(10);
    while feed.queued() < RATE as usize && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(feed.queued(), RATE as usize, "the thread filled the ring");
}

#[test]
fn a_loop_past_the_file_s_end_ends_the_stream_rather_than_spinning() {
    // A clip whose length was not declared, looping from a frame the file
    // does not reach: the decoder must not turn back forever.
    let real = ramp_clip(100);
    let source = real.stream_source().expect("streamed").clone();
    let clip = Clip::streamed(
        source,
        StreamInfo {
            channels: 1,
            rate: RATE,
            frames: None,
        },
    )
    .with_loop(500, None);
    let (mut decoder, _feed) = start(&clip, true).expect("the stream opens");

    let mut rounds = 0;
    while decoder.fill() == Progress::Running && rounds < 100 {
        rounds += 1;
    }

    assert!(decoder.is_done(), "it gave up on the empty loop");
}

#[test]
fn a_file_that_is_not_audio_does_not_probe() {
    let source = StreamSource::Bytes {
        path: PathBuf::from("notes.wav"),
        bytes: Arc::from(&b"not a wav at all"[..]),
    };
    assert!(probe(source).is_err());
}

#[test]
fn a_file_that_is_not_there_does_not_probe() {
    let source = StreamSource::File(PathBuf::from("/nowhere/at/all.ogg"));
    assert!(matches!(probe(source), Err(AudioError::Read { .. })));
}
