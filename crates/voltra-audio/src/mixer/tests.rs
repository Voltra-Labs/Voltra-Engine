use super::*;
use std::sync::mpsc::channel;

const RATE: u32 = 8;

fn id(raw: u64) -> VoiceId {
    VoiceId::forge(raw)
}

/// A clip of `frames` samples, every one of them `0.5`, at the mixer's own
/// rate so one output frame is one clip frame.
///
/// Half scale so one voice sits under the limiter's threshold and the
/// tests read the mix arithmetic rather than the limiter's.
fn tone(frames: usize) -> Clip {
    Clip::new(vec![0.5; frames], 1, RATE)
}

/// Hard left, so a voice's contribution lands in channel 0 at full gain
/// and the test reads the arithmetic rather than the pan law.
fn left() -> PlayParams {
    PlayParams {
        pan: -1.0,
        ..Default::default()
    }
}

fn play(mixer: &mut Mixer, raw: u64, frames: usize) -> VoiceId {
    let id = id(raw);
    mixer.apply(Command::Play {
        id,
        clip: tone(frames),
        params: left(),
        fade_in: Duration::ZERO,
        feed: None,
        curve: Curve::Linear,
    });
    id
}

fn stop(id: VoiceId) -> Command {
    Command::Stop {
        id,
        over: Duration::ZERO,
        curve: Curve::Linear,
    }
}

fn render(mixer: &mut Mixer, frames: usize) -> Vec<f32> {
    let mut out = vec![0.0; frames * usize::from(mixer.channels())];
    mixer.render(&mut out);
    out
}

#[test]
fn a_mixer_with_nothing_playing_renders_silence() {
    let mut mixer = Mixer::new(RATE, 2);
    assert_eq!(render(&mut mixer, 2), vec![0.0; 4]);
}

#[test]
fn a_render_overwrites_whatever_the_device_left_in_the_buffer() {
    // The bug a buffer that is only added into leaves: the previous
    // callback's audio plays again under the silence.
    let mut mixer = Mixer::new(RATE, 2);
    let mut out = vec![0.9; 4];
    mixer.render(&mut out);
    assert_eq!(out, vec![0.0; 4]);
}

#[test]
fn two_voices_sum() {
    let mut mixer = Mixer::new(RATE, 2);
    mixer.apply(Command::Play {
        id: id(1),
        clip: Clip::new(vec![0.25, 0.25], 1, RATE),
        params: left(),
        fade_in: Duration::ZERO,
        feed: None,
        curve: Curve::Linear,
    });
    mixer.apply(Command::Play {
        id: id(2),
        clip: Clip::new(vec![0.5, 0.5], 1, RATE),
        params: left(),
        fade_in: Duration::ZERO,
        feed: None,
        curve: Curve::Linear,
    });

    let out = render(&mut mixer, 2);
    assert!((out[0] - 0.75).abs() < 1e-6, "got {out:?}");
}

#[test]
fn the_sum_never_leaves_the_range_a_device_accepts() {
    let mut mixer = Mixer::new(RATE, 2);
    for raw in 0..4 {
        play(&mut mixer, raw, 2);
    }
    let out = render(&mut mixer, 2);
    assert!(out.iter().all(|s| (-1.0..=1.0).contains(s)), "got {out:?}");
}

#[test]
fn a_voice_that_runs_out_is_dropped() {
    let mut mixer = Mixer::new(RATE, 2);
    play(&mut mixer, 1, 2);
    assert_eq!(mixer.voices(), 1);

    render(&mut mixer, 4);

    assert_eq!(mixer.voices(), 0, "nothing should still be sounding");
}

#[test]
fn stopping_a_voice_takes_it_out_of_the_mix() {
    let mut mixer = Mixer::new(RATE, 2);
    let voice = play(&mut mixer, 1, 64);

    mixer.apply(stop(voice));

    assert_eq!(mixer.voices(), 0);
    assert_eq!(render(&mut mixer, 2), vec![0.0; 4]);
}

#[test]
fn stopping_a_voice_that_already_finished_is_not_an_error() {
    // A game holding an id cannot know the clip ran out, and ids are never
    // reused, so this must be a no-op rather than hitting something else.
    let mut mixer = Mixer::new(RATE, 2);
    let voice = play(&mut mixer, 1, 1);
    render(&mut mixer, 4);
    let survivor = play(&mut mixer, 2, 64);

    mixer.apply(stop(voice));

    assert_eq!(mixer.voices(), 1);
    mixer.apply(Command::SetGain {
        id: survivor,
        volume: 1.0,
        pan: -1.0,
    });
}

#[test]
fn stop_all_ends_everything() {
    let mut mixer = Mixer::new(RATE, 2);
    play(&mut mixer, 1, 64);
    play(&mut mixer, 2, 64);

    mixer.apply(Command::StopAll {
        over: Duration::ZERO,
    });

    assert_eq!(mixer.voices(), 0);
}

#[test]
fn a_gain_change_reaches_the_voice_it_names() {
    let mut mixer = Mixer::new(RATE, 2);
    let voice = play(&mut mixer, 1, 64);

    mixer.apply(Command::SetGain {
        id: voice,
        volume: 1.0,
        pan: 1.0,
    });

    let out = render(&mut mixer, 1);
    assert!(out[0].abs() < 1e-6, "it should have moved right: {out:?}");
    assert!((out[1] - 0.5).abs() < 1e-6);
}

#[test]
fn a_gain_change_for_an_unknown_voice_touches_nothing() {
    let mut mixer = Mixer::new(RATE, 2);
    play(&mut mixer, 1, 64);

    mixer.apply(Command::SetGain {
        id: id(99),
        volume: 0.0,
        pan: 0.0,
    });

    let out = render(&mut mixer, 1);
    assert!((out[0] - 0.5).abs() < 1e-6, "the real voice changed");
}

#[test]
fn a_full_mixer_steals_the_oldest_voice() {
    let mut mixer = Mixer::with_capacity(RATE, 2, 2);
    let oldest = play(&mut mixer, 1, 64);
    play(&mut mixer, 2, 64);

    play(&mut mixer, 3, 64);

    assert_eq!(mixer.voices(), 2, "the ceiling holds");
    mixer.apply(stop(oldest));
    assert_eq!(mixer.voices(), 2, "and it was the oldest that went");
}

#[test]
fn a_capacity_of_zero_still_plays_one_voice() {
    // A caller that computed a ceiling and got zero must not be handed a
    // mixer that silently drops everything.
    let mut mixer = Mixer::with_capacity(RATE, 2, 0);
    play(&mut mixer, 1, 64);
    assert_eq!(mixer.voices(), 1);
}

#[test]
fn a_finished_voice_hands_its_clip_back_to_be_dropped_elsewhere() {
    // The reason the channel exists: freeing the samples must not happen
    // inside the device callback.
    let (sender, receiver) = channel();
    let mut mixer = Mixer::new(RATE, 2).retiring_to(sender);
    play(&mut mixer, 1, 2);

    render(&mut mixer, 4);

    let returned = receiver.try_recv().expect("the clip should come back");
    assert_eq!(returned.clip().map(Clip::frames), Some(2));
}

#[test]
fn a_stolen_voice_hands_its_clip_back_too() {
    let (sender, receiver) = channel();
    let mut mixer = Mixer::with_capacity(RATE, 2, 1).retiring_to(sender);
    play(&mut mixer, 1, 64);
    play(&mut mixer, 2, 64);

    assert!(receiver.try_recv().is_ok());
}

#[test]
fn a_closed_retirement_channel_does_not_stop_the_mix() {
    // The game went away first. The callback has to keep working until the
    // device is torn down.
    let (sender, receiver) = channel();
    let mut mixer = Mixer::new(RATE, 2).retiring_to(sender);
    drop(receiver);
    play(&mut mixer, 1, 1);

    render(&mut mixer, 4);

    assert_eq!(mixer.voices(), 0);
}

#[test]
fn a_degenerate_device_description_is_floored_rather_than_dividing_by_zero() {
    let mixer = Mixer::new(0, 0);
    assert_eq!(mixer.rate(), 1);
    assert_eq!(mixer.channels(), 1);
}

/// A long voice on `bus`, hard left.
fn play_on(mixer: &mut Mixer, raw: u64, bus: BusId) -> VoiceId {
    let id = id(raw);
    mixer.apply(Command::Play {
        id,
        clip: tone(64),
        params: PlayParams { bus, ..left() },
        fade_in: Duration::ZERO,
        feed: None,
        curve: Curve::Linear,
    });
    id
}

fn set_bus(mixer: &mut Mixer, bus: BusId, volume: f32) {
    mixer.apply(Command::SetBusVolume {
        bus,
        volume,
        over: Duration::ZERO,
    });
}

#[test]
fn a_bus_s_volume_reaches_only_the_voices_on_it() {
    let music = BusId::from_index(1);
    let mut mixer = Mixer::new(RATE, 2);
    play_on(&mut mixer, 1, music);
    set_bus(&mut mixer, music, 0.0);

    assert_eq!(render(&mut mixer, 1), vec![0.0, 0.0], "the music is muted");

    play_on(&mut mixer, 2, BusId::from_index(2));
    let out = render(&mut mixer, 1);
    assert!((out[0] - 0.5).abs() < 1e-6, "the effects are not: {out:?}");
}

#[test]
fn the_master_turns_down_everything() {
    let mut mixer = Mixer::new(RATE, 2);
    play_on(&mut mixer, 1, BusId::from_index(1));
    play_on(&mut mixer, 2, BusId::MASTER);
    set_bus(&mut mixer, BusId::MASTER, 0.25);

    let out = render(&mut mixer, 1);

    assert!((out[0] - 0.25).abs() < 1e-6, "got {out:?}");
}

#[test]
fn a_bus_fade_arrives_on_time_even_with_nothing_on_the_bus() {
    let music = BusId::from_index(1);
    let mut mixer = Mixer::new(1_000, 2);
    mixer.apply(Command::SetBusVolume {
        bus: music,
        volume: 0.0,
        over: Duration::from_millis(10),
    });

    let mut out = vec![0.0; 20];
    mixer.render(&mut out);

    assert_eq!(mixer.bus_volume(music), 0.0);
}

#[test]
fn a_fading_stop_keeps_the_voice_until_it_is_silent() {
    let mut mixer = Mixer::new(1_000, 2);
    mixer.apply(Command::Play {
        id: id(1),
        clip: Clip::new(vec![0.5; 1_000], 1, 1_000),
        params: left(),
        fade_in: Duration::ZERO,
        feed: None,
        curve: Curve::Linear,
    });
    mixer.apply(Command::Stop {
        id: id(1),
        over: Duration::from_millis(4),
        curve: Curve::Linear,
    });
    assert_eq!(mixer.voices(), 1, "a fade-out is still sounding");

    let mut out = vec![0.0; 2 * 8];
    mixer.render(&mut out);

    assert_eq!(mixer.voices(), 0);
    assert!(out[0] > 0.0 && out[6] < out[0], "it faded: {out:?}");
}

#[test]
fn a_fade_reaches_the_voice_it_names() {
    let mut mixer = Mixer::new(RATE, 2);
    let voice = play(&mut mixer, 1, 64);
    mixer.apply(Command::Fade {
        id: voice,
        volume: 0.0,
        over: Duration::ZERO,
        curve: Curve::Linear,
    });

    assert_eq!(render(&mut mixer, 1), vec![0.0, 0.0]);
    assert_eq!(mixer.voices(), 1, "a fade to silence is not a stop");
}

#[test]
fn a_bus_id_past_the_table_is_ignored_rather_than_panicking() {
    let mut mixer = Mixer::new(RATE, 2);
    // Only reachable by forging a command; `Buses` caps what it issues.
    let wild = BusId::from_index(MAX_BUSES - 1);
    set_bus(&mut mixer, wild, 0.5);
    play_on(&mut mixer, 1, wild);
    render(&mut mixer, 1);
    assert_eq!(mixer.bus_volume(wild), 0.5);
}

#[test]
fn an_equal_power_cross_fade_keeps_the_sum_s_power_constant() {
    // The outgoing track hard left, the incoming one hard right, both a
    // constant 0.5: each side's level is then one voice's fader, and the
    // power of the pair must not dip anywhere across the fade.
    let mut mixer = Mixer::new(1_000, 2);
    let constant = || Clip::new(vec![0.5; 1_000], 1, 1_000);
    mixer.apply(Command::Play {
        id: id(1),
        clip: constant(),
        feed: None,
        params: left(),
        fade_in: Duration::ZERO,
        curve: Curve::Linear,
    });
    render(&mut mixer, 10);

    let over = Duration::from_millis(200);
    mixer.apply(Command::Stop {
        id: id(1),
        over,
        curve: Curve::EqualPower,
    });
    mixer.apply(Command::Play {
        id: id(2),
        clip: constant(),
        feed: None,
        params: PlayParams {
            pan: 1.0,
            ..Default::default()
        },
        fade_in: over,
        curve: Curve::EqualPower,
    });

    let out = render(&mut mixer, 250);
    for (frame, pair) in out.chunks(2).enumerate() {
        let power = (pair[0] / 0.5).powi(2) + (pair[1] / 0.5).powi(2);
        assert!(
            (power - 1.0).abs() < 1e-3,
            "frame {frame}: {pair:?} has power {power}"
        );
    }
    assert!(out[0] > 0.49 && out[1] < 0.01, "it starts on the old track");
    assert!(
        out[498] < 0.01 && out[499] > 0.49,
        "and ends on the new one"
    );
    assert_eq!(mixer.voices(), 1, "the old voice is gone once faded");
}
