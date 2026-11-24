//! Each effect measured through the mixer, offline: tones in, levels out.

use std::f32::consts::TAU;
use std::sync::mpsc::channel;
use std::time::Duration;

use super::*;
use crate::bus::BusId;
use crate::clip::Clip;
use crate::command::Command;
use crate::mixer::{Mixer, BLOCK};
use crate::params::PlayParams;
use crate::ramp::Curve;
use crate::retired::Retired;
use crate::voice::VoiceId;

const RATE: u32 = 48_000;

fn music() -> BusId {
    BusId::from_index(1)
}

fn dialogue() -> BusId {
    BusId::from_index(2)
}

/// A table where `music` is bus 1 and `dialogue` bus 2, as the tests name
/// them.
fn buses() -> Buses {
    Buses::new(["music", "dialogue"])
}

/// A sine of `hertz` at amplitude 0.5, `seconds` long.
fn sine(hertz: f32, seconds: f32) -> Clip {
    let frames = (seconds * RATE as f32) as usize;
    let samples: Vec<f32> = (0..frames)
        .map(|i| 0.5 * (TAU * hertz * i as f32 / RATE as f32).sin())
        .collect();
    Clip::new(samples, 1, RATE)
}

/// One sample of 0.5, then a second of silence.
fn impulse() -> Clip {
    let mut samples = vec![0.0; RATE as usize];
    samples[0] = 0.5;
    Clip::new(samples, 1, RATE)
}

/// A constant `level`, `seconds` long.
fn constant(level: f32, seconds: f32) -> Clip {
    Clip::new(vec![level; (seconds * RATE as f32) as usize], 1, RATE)
}

/// A stereo mixer at 48 kHz with `effects` on the music bus.
fn mixer_with(effects: &[EffectDesc]) -> Mixer {
    let mut mixer = Mixer::new(RATE, 2);
    let chain = Effect::build_chain(effects, RATE, &mut buses());
    mixer.apply(Command::SetBusEffects {
        bus: music(),
        effects: chain,
    });
    mixer
}

/// Plays `clip` on `bus`, panned by `pan`.
fn play(mixer: &mut Mixer, raw: u64, clip: Clip, bus: BusId, pan: f32) {
    mixer.apply(Command::Play {
        id: VoiceId::forge(raw),
        clip,
        feed: None,
        params: PlayParams {
            pan,
            bus,
            ..Default::default()
        },
        fade_in: Duration::ZERO,
        curve: Curve::Linear,
    });
}

/// Renders `frames` frames a block at a time and returns the left channel.
fn left(mixer: &mut Mixer, frames: usize) -> Vec<f32> {
    let mut heard = Vec::with_capacity(frames);
    let mut out = vec![0.0; BLOCK * 2];
    while heard.len() < frames {
        mixer.render(&mut out);
        heard.extend(out.iter().step_by(2));
    }
    heard.truncate(frames);
    heard
}

fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len().max(1) as f32).sqrt()
}

/// How loud a sine of `hertz` comes out of `effect` relative to going in,
/// measured over the second half of half a second so the filter has
/// settled.
fn response(effect: EffectDesc, hertz: f32) -> f32 {
    let mut mixer = mixer_with(&[effect]);
    play(&mut mixer, 1, sine(hertz, 0.6), music(), -1.0);
    let heard = left(&mut mixer, RATE as usize / 2);
    rms(&heard[RATE as usize / 4..]) / (0.5 / 2f32.sqrt())
}

fn decibels(ratio: f32) -> f32 {
    20.0 * ratio.max(1e-9).log10()
}

#[test]
fn a_low_pass_keeps_what_is_below_its_cutoff_and_takes_what_is_above() {
    let low = |hertz| {
        response(
            EffectDesc::LowPass {
                cutoff: 1_000.0,
                q: std::f32::consts::FRAC_1_SQRT_2,
            },
            hertz,
        )
    };
    let below = decibels(low(200.0));
    let at = decibels(low(1_000.0));
    let above = decibels(low(8_000.0));
    assert!(below > -0.5, "200 Hz should pass: {below} dB");
    assert!((at + 3.0).abs() < 0.5, "the cutoff is -3 dB: {at} dB");
    // Three octaves past a 12 dB-per-octave slope.
    assert!(above < -30.0, "8 kHz should be gone: {above} dB");
}

#[test]
fn a_high_pass_keeps_what_is_above_its_cutoff_and_takes_what_is_below() {
    let high = |hertz| {
        response(
            EffectDesc::HighPass {
                cutoff: 1_000.0,
                q: std::f32::consts::FRAC_1_SQRT_2,
            },
            hertz,
        )
    };
    let above = decibels(high(8_000.0));
    let at = decibels(high(1_000.0));
    let below = decibels(high(125.0));
    assert!(above > -0.5, "8 kHz should pass: {above} dB");
    assert!((at + 3.0).abs() < 0.5, "the cutoff is -3 dB: {at} dB");
    assert!(below < -30.0, "125 Hz should be gone: {below} dB");
}

#[test]
fn a_cutoff_ramped_from_a_tick_opens_the_filter() {
    let mut mixer = mixer_with(&[EffectDesc::LowPass {
        cutoff: 200.0,
        q: std::f32::consts::FRAC_1_SQRT_2,
    }]);
    play(&mut mixer, 1, sine(4_000.0, 1.0), music(), -1.0);
    let closed = rms(&left(&mut mixer, RATE as usize / 10)[2_400..]);

    mixer.apply(Command::SetBusParam {
        bus: music(),
        effect: 0,
        param: Param::Cutoff,
        value: 18_000.0,
        over: Duration::from_millis(100),
    });
    let ramp = left(&mut mixer, RATE as usize / 10);
    let open = rms(&left(&mut mixer, RATE as usize / 10));

    let full = 0.5 / 2f32.sqrt();
    assert!(
        closed < full * 0.05,
        "4 kHz under a 200 Hz cutoff: {closed}"
    );
    assert!(open > full * 0.95, "and through an 18 kHz one: {open}");
    let middle = rms(&ramp[ramp.len() / 2..]);
    assert!(
        middle > closed && middle < open * 1.01,
        "it opened along the way: {closed} < {middle} < {open}"
    );
}

#[test]
fn a_delay_echoes_after_its_time_and_feeds_back() {
    let mut mixer = mixer_with(&[EffectDesc::Delay {
        time: 0.25,
        feedback: 0.5,
        mix: 0.5,
    }]);
    play(&mut mixer, 1, impulse(), music(), -1.0);

    let heard = left(&mut mixer, RATE as usize);

    let echo = RATE as usize / 4;
    assert!((heard[0] - 0.25).abs() < 1e-6, "the dry half: {}", heard[0]);
    let loudest_after = |from: usize, to: usize| {
        (from..to)
            .max_by(|&a, &b| heard[a].abs().total_cmp(&heard[b].abs()))
            .expect("a range")
    };
    assert_eq!(loudest_after(1, echo + echo / 2), echo, "the echo's time");
    assert!(
        (heard[echo] - 0.25).abs() < 1e-6,
        "the wet half: {}",
        heard[echo]
    );
    assert_eq!(loudest_after(echo + 1, 2 * echo + echo / 2), 2 * echo);
    assert!(
        (heard[2 * echo] - 0.125).abs() < 1e-6,
        "the second echo at half: {}",
        heard[2 * echo]
    );
    assert!(
        heard[1..echo].iter().all(|s| s.abs() < 1e-6),
        "silence between"
    );
}

/// The RMS of a reverb's answer to an impulse over `from..to` seconds.
fn tail(room_size: f32, from: f32, to: f32) -> f32 {
    let mut mixer = mixer_with(&[EffectDesc::Reverb {
        room_size,
        damping: 0.5,
        width: 1.0,
        mix: 1.0,
    }]);
    play(&mut mixer, 1, impulse(), music(), -1.0);
    let heard = left(&mut mixer, (to * RATE as f32) as usize);
    rms(&heard[(from * RATE as f32) as usize..])
}

#[test]
fn a_reverb_rings_after_the_sound_and_its_tail_decays() {
    let early = tail(0.5, 0.05, 0.15);
    let middle = tail(0.5, 1.0, 1.1);
    let late = tail(0.5, 2.0, 2.1);
    assert!(early > 1e-4, "the room answers: {early}");
    assert!(middle < early, "{middle} after {early}");
    assert!(late < middle * 0.5, "{late} after {middle}");
    assert!(
        decibels(late / early) < -40.0,
        "down {} dB in two seconds",
        decibels(late / early)
    );
}

#[test]
fn a_bigger_room_rings_longer() {
    let small = tail(0.2, 1.0, 1.1);
    let big = tail(0.9, 1.0, 1.1);
    assert!(big > small * 10.0, "big {big}, small {small}");
}

#[test]
fn a_tail_follows_the_bus_s_volume() {
    // The fader is after the inserts, as on a desk.
    let mut mixer = mixer_with(&[EffectDesc::Reverb {
        room_size: 0.9,
        damping: 0.2,
        width: 1.0,
        mix: 1.0,
    }]);
    play(&mut mixer, 1, impulse(), music(), -1.0);
    left(&mut mixer, BLOCK * 4);
    mixer.apply(Command::SetBusVolume {
        bus: music(),
        volume: 0.0,
        over: Duration::ZERO,
    });
    assert!(left(&mut mixer, BLOCK * 4).iter().all(|s| *s == 0.0));
}

#[test]
fn a_duck_pulls_its_bus_down_by_its_depth_over_its_attack_and_back_over_its_release() {
    let mut mixer = mixer_with(&[EffectDesc::Duck {
        by: "dialogue".to_owned(),
        depth: 0.75,
        threshold: 0.01,
        attack: 0.05,
        release: 0.2,
    }]);
    // The music on the left, the dialogue on the right, so the left
    // channel is the music alone.
    play(&mut mixer, 1, constant(0.4, 3.0), music(), -1.0);
    let start = BLOCK * 94;
    let before = left(&mut mixer, start);
    play(&mut mixer, 2, constant(0.4, 0.5), dialogue(), 1.0);
    // Whole blocks, so nothing rendered is left out of what is measured.
    let during = left(
        &mut mixer,
        (RATE as usize / 2).div_ceil(BLOCK) * BLOCK + BLOCK,
    );
    let after = left(&mut mixer, RATE as usize / 2);

    assert!(before.iter().all(|s| (s - 0.4).abs() < 1e-6), "undisturbed");

    let attack = (0.05 * RATE as f32) as usize;
    let expect = |gain: f32| 0.4 * gain;
    assert!((during[attack / 2 - 1] - expect(1.0 - 0.375)).abs() < 1e-4);
    assert!(during[attack - 2] > expect(0.25), "not down yet");
    assert!(
        (during[attack - 1] - expect(0.25)).abs() < 1e-5,
        "a quarter left once the attack is over: {}",
        during[attack - 1]
    );
    assert!(during[attack..RATE as usize / 2 - BLOCK]
        .iter()
        .all(|s| (s - expect(0.25)).abs() < 1e-5));

    // The dialogue ends inside a block; the block after is the first that
    // hears nothing, and the release starts there.
    let end = start + RATE as usize / 2;
    let quiet = end.div_ceil(BLOCK) * BLOCK;
    let release = (0.2 * RATE as f32) as usize;
    let all: Vec<f32> = before
        .iter()
        .chain(&during)
        .chain(&after)
        .copied()
        .collect();
    assert!((all[quiet - 1] - expect(0.25)).abs() < 1e-5, "still down");
    assert!(
        (all[quiet + release / 2 - 1] - expect(0.625)).abs() < 1e-4,
        "half way back: {}",
        all[quiet + release / 2 - 1]
    );
    assert!((all[quiet + release - 1] - 0.4).abs() < 1e-5, "all the way");
}

#[test]
fn effects_on_one_bus_leave_the_others_alone() {
    let mut mixer = mixer_with(&[EffectDesc::LowPass {
        cutoff: 100.0,
        q: 0.7,
    }]);
    play(&mut mixer, 1, sine(6_000.0, 0.3), dialogue(), -1.0);
    let heard = left(&mut mixer, RATE as usize / 5);
    let level = rms(&heard[RATE as usize / 10..]) / (0.5 / 2f32.sqrt());
    assert!(level > 0.99, "the dialogue is not filtered: {level}");
}

#[test]
fn a_replaced_chain_goes_back_to_be_freed() {
    let (sender, receiver) = channel();
    let mut mixer = Mixer::new(RATE, 2).retiring_to(sender);
    let chain = |cutoff| {
        Effect::build_chain(
            &[EffectDesc::LowPass { cutoff, q: 0.7 }],
            RATE,
            &mut buses(),
        )
    };
    mixer.apply(Command::SetBusEffects {
        bus: music(),
        effects: chain(500.0),
    });
    mixer.apply(Command::SetBusEffects {
        bus: music(),
        effects: chain(800.0),
    });

    assert!(matches!(receiver.try_recv(), Ok(Retired::Effects(old)) if old.len() == 1));
    assert_eq!(mixer.bus_effects(music()).len(), 1);
}

#[test]
fn a_parameter_an_effect_does_not_have_changes_nothing() {
    let mut delay = Effect::build(
        &EffectDesc::Delay {
            time: 0.1,
            feedback: 0.0,
            mix: 0.5,
        },
        RATE,
        &mut buses(),
    );
    assert!(!delay.set(Param::Cutoff, 100.0, 0));
    assert!(delay.set(Param::Time, 0.2, 0));
}

#[test]
fn a_chain_longer_than_the_ceiling_keeps_its_first_effects() {
    let many = vec![
        EffectDesc::LowPass {
            cutoff: 9_000.0,
            q: 0.7
        };
        MAX_EFFECTS + 3
    ];
    assert_eq!(
        Effect::build_chain(&many, RATE, &mut buses()).len(),
        MAX_EFFECTS
    );
}
