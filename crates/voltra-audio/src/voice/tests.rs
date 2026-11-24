use super::*;

const ID: VoiceId = VoiceId(1);

/// A voice over `samples` with the device running at the clip's own rate,
/// so one output frame is one clip frame and the arithmetic is visible.
fn voice(samples: Vec<f32>, channels: u16, params: PlayParams) -> Voice {
    Voice::new(
        ID,
        Clip::new(samples, channels, 8),
        params,
        8,
        Duration::ZERO,
    )
}

/// Hard left, so a mono voice writes its raw samples into channel 0 and
/// nothing into channel 1 — the pan law's `1.0` end.
fn left() -> PlayParams {
    PlayParams {
        pan: -1.0,
        ..Default::default()
    }
}

fn mix(voice: &mut Voice, frames: usize, channels: usize) -> Vec<f32> {
    let mut out = vec![0.0; frames * channels];
    voice.mix(&mut out, channels);
    out
}

#[test]
fn a_voice_at_the_device_s_own_rate_plays_its_samples_untouched() {
    let mut voice = voice(vec![0.25, 0.5, 0.75], 1, left());
    let out = mix(&mut voice, 3, 2);
    assert_eq!(out, vec![0.25, 0.0, 0.5, 0.0, 0.75, 0.0]);
}

#[test]
fn a_voice_adds_into_the_buffer_rather_than_replacing_it() {
    // What makes a mixer a mixer: two voices in one buffer.
    let mut voice = voice(vec![0.25, 0.25], 1, left());
    let mut out = vec![1.0, 0.0, 1.0, 0.0];
    voice.mix(&mut out, 2);
    assert_eq!(out, vec![1.25, 0.0, 1.25, 0.0]);
}

#[test]
fn a_stereo_clip_keeps_its_sides() {
    let params = PlayParams::default();
    let mut voice = voice(vec![1.0, -1.0, 0.5, -0.5], 2, params);
    let out = mix(&mut voice, 2, 2);

    let (gain_left, gain_right) = params.gains();
    assert!((out[0] - gain_left).abs() < 1e-6);
    assert!((out[1] + gain_right).abs() < 1e-6);
}

#[test]
fn a_mono_clip_reaches_both_sides_when_it_is_centred() {
    let mut voice = voice(vec![1.0], 1, PlayParams::default());
    let out = mix(&mut voice, 1, 2);
    assert!((out[0] - out[1]).abs() < 1e-6);
    assert!(out[0] > 0.0);
}

#[test]
fn a_mono_device_sums_the_two_sides() {
    let mut voice = voice(vec![1.0], 1, PlayParams::default());
    let out = mix(&mut voice, 1, 1);
    // Both sides at 0.707, halved: the same loudness a stereo device gets
    // out of its pair.
    assert!((out[0] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
}

#[test]
fn a_voice_that_runs_out_leaves_the_rest_of_the_buffer_alone() {
    // The buffer is longer than the sound, which is every callback that
    // contains the end of a one-shot.
    let mut voice = voice(vec![1.0, 1.0], 1, left());
    let out = mix(&mut voice, 4, 2);
    assert_eq!(&out[4..], &[0.0, 0.0, 0.0, 0.0]);
    assert!(voice.is_finished());
}

#[test]
fn a_finished_voice_stays_finished_and_adds_nothing_more() {
    let mut voice = voice(vec![1.0], 1, left());
    mix(&mut voice, 4, 2);
    let again = mix(&mut voice, 4, 2);
    assert_eq!(again, vec![0.0; 8]);
}

#[test]
fn an_empty_clip_is_finished_immediately_even_when_it_loops() {
    // A file that would not decode, played by a source set to loop. The
    // voice must not occupy a slot forever.
    let looping = PlayParams {
        looping: true,
        ..Default::default()
    };
    let mut voice = voice(Vec::new(), 1, looping);
    assert!(voice.is_finished());
    assert_eq!(mix(&mut voice, 4, 2), vec![0.0; 8]);
}

#[test]
fn a_looping_voice_starts_again_at_the_end() {
    let looping = PlayParams {
        looping: true,
        ..left()
    };
    let mut voice = voice(vec![0.5, 0.25], 1, looping);
    let out = mix(&mut voice, 4, 2);
    assert_eq!(out, vec![0.5, 0.0, 0.25, 0.0, 0.5, 0.0, 0.25, 0.0]);
    assert!(!voice.is_finished());
}

#[test]
fn a_looping_voice_pitched_past_the_whole_clip_still_wraps() {
    // The bug a subtraction instead of a modulo leaves behind: one step
    // longer than the clip and the cursor never comes back.
    let looping = PlayParams {
        looping: true,
        pitch: 8.0,
        ..left()
    };
    let mut voice = voice(vec![1.0, 1.0, 1.0, 1.0], 1, looping);
    let out = mix(&mut voice, 4, 2);
    assert!(out.iter().any(|s| *s != 0.0), "it went silent: {out:?}");
    assert!(!voice.is_finished());
}

#[test]
fn a_faster_pitch_reads_further_into_the_clip() {
    let fast = PlayParams {
        pitch: 2.0,
        ..left()
    };
    let mut voice = voice(vec![0.0, 0.1, 0.2, 0.3], 1, fast);
    let out = mix(&mut voice, 2, 2);
    assert!((out[0] - 0.0).abs() < 1e-6);
    assert!((out[2] - 0.2).abs() < 1e-6, "got {out:?}");
}

#[test]
fn a_device_running_faster_than_the_clip_interpolates_between_frames() {
    // The everyday case: a 22 kHz file on a 44 kHz device.
    let clip = Clip::new(vec![0.0, 1.0], 1, 8);
    let mut voice = Voice::new(ID, clip, left(), 16, Duration::ZERO);
    let out = mix(&mut voice, 2, 2);
    assert!((out[0] - 0.0).abs() < 1e-6);
    assert!((out[2] - 0.5).abs() < 1e-6, "got {out:?}");
}

#[test]
fn a_pitch_of_zero_holds_the_first_frame_rather_than_finishing() {
    let held = PlayParams {
        pitch: 0.0,
        ..left()
    };
    let mut voice = voice(vec![0.75, 0.1], 1, held);
    let out = mix(&mut voice, 3, 2);
    assert_eq!(out, vec![0.75, 0.0, 0.75, 0.0, 0.75, 0.0]);
}

#[test]
fn moving_a_voice_changes_its_gains_without_restarting_it() {
    let mut voice = voice(vec![1.0, 1.0, 1.0], 1, left());
    mix(&mut voice, 1, 2);

    voice.set_gain(1.0, 1.0);
    let out = mix(&mut voice, 1, 2);

    assert!(out[0].abs() < 1e-6, "it moved to the other ear");
    assert!((out[1] - 1.0).abs() < 1e-6);
    assert!(!voice.is_finished(), "and did not start over");
}

#[test]
fn a_zero_channel_buffer_is_ignored_rather_than_dividing_by_zero() {
    let mut voice = voice(vec![1.0], 1, left());
    let mut out = Vec::new();
    voice.mix(&mut out, 0);
}

/// A voice at 1 kHz over a constant clip, so fades are a few frames long
/// and readable.
fn slow(params: PlayParams, fade_in: Duration) -> Voice {
    Voice::new(
        ID,
        Clip::new(vec![1.0; 64], 1, 1_000),
        params,
        1_000,
        fade_in,
    )
}

#[test]
fn a_fade_in_rises_from_silence() {
    let mut voice = slow(left(), Duration::from_millis(4));
    let out = mix(&mut voice, 5, 2);
    let heard: Vec<f32> = out.iter().step_by(2).copied().collect();
    assert_eq!(heard, vec![0.0, 0.25, 0.5, 0.75, 1.0]);
}

#[test]
fn a_fade_out_ends_the_voice_where_it_arrives() {
    let mut voice = slow(left(), Duration::ZERO);
    voice.stop(Duration::from_millis(2), Curve::Linear);
    assert!(!voice.is_finished(), "it is still fading");

    let out = mix(&mut voice, 4, 2);

    assert!(voice.is_finished());
    assert_eq!(out, vec![1.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0]);
}

#[test]
fn a_fade_cancels_a_stop_in_progress() {
    let mut voice = slow(left(), Duration::ZERO);
    voice.stop(Duration::from_millis(4), Curve::Linear);
    mix(&mut voice, 2, 2);

    voice.fade(1.0, Duration::from_millis(2), Curve::Linear);
    mix(&mut voice, 8, 2);

    assert!(!voice.is_finished(), "the game changed its mind");
}

#[test]
fn a_gain_change_slides_rather_than_steps() {
    // At 1 kHz the smoothing is fifteen frames: the first frame after the
    // change must still be near the old gain, not at the new one.
    let mut voice = slow(left(), Duration::ZERO);
    voice.set_gain(0.0, -1.0);

    let out = mix(&mut voice, 16, 2);

    assert!(out[0] > 0.9, "it jumped: {}", out[0]);
    assert_eq!(out[30], 0.0, "and it arrives");
}

#[test]
fn durations_become_frames_at_the_device_s_rate() {
    assert_eq!(frames(Duration::from_millis(15), 48_000), 720);
    assert_eq!(frames(Duration::ZERO, 48_000), 0);
    assert_eq!(frames(Duration::from_secs(u64::MAX), 48_000), u32::MAX);
}

#[test]
fn a_looping_voice_plays_its_intro_once_then_the_loop() {
    // Frames 0 and 1 are the intro; 2..4 is the loop.
    let clip = Clip::new(vec![0.1, 0.2, 0.3, 0.4, 0.9], 1, 8).with_loop(2, Some(4));
    let looping = PlayParams {
        looping: true,
        ..left()
    };
    let mut voice = Voice::new(ID, clip, looping, 8, Duration::ZERO);
    let out = mix(&mut voice, 7, 2);
    let heard: Vec<f32> = out.iter().step_by(2).copied().collect();
    assert_eq!(heard, vec![0.1, 0.2, 0.3, 0.4, 0.3, 0.4, 0.3]);
}

#[test]
fn a_voice_that_does_not_loop_plays_past_the_loop_to_the_end() {
    let clip = Clip::new(vec![0.1, 0.2, 0.3, 0.4, 0.9], 1, 8).with_loop(2, Some(4));
    let mut voice = Voice::new(ID, clip, left(), 8, Duration::ZERO);
    let out = mix(&mut voice, 5, 2);
    assert_eq!(out[8], 0.9);
    assert!(voice.is_finished());
}

#[test]
fn a_streamed_clip_with_no_feed_is_finished_rather_than_stuck() {
    // The stream could not be opened; the slot must not be held forever.
    let source = std::sync::Arc::new(crate::stream::StreamSource::File("gone.ogg".into()));
    let clip = Clip::streamed(
        source,
        crate::stream::StreamInfo {
            channels: 1,
            rate: 8,
            frames: None,
        },
    );
    let looping = PlayParams {
        looping: true,
        ..left()
    };
    let voice = Voice::new(ID, clip, looping, 8, Duration::ZERO);
    assert!(voice.is_finished());
}
