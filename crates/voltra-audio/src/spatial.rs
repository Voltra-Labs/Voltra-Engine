//! Where a sound is, turned into how it is mixed.
//!
//! Two functions and no state: the caller knows where the listener and the
//! source are — that is the scene's business — and this knows what an offset
//! between them means for a gain and a pan. Keeping it here rather than in
//! `voltra-scene` is what lets the falloff be tested without a world, and what
//! stops the same arithmetic being written twice when a second caller (a
//! prefab spawning a sound, a tilemap of ambience) wants it.
//!
//! Plain `f32`s rather than a `Vec2` on purpose: this crate owns the audio
//! backend and nothing else, and a vector type is the scene's vocabulary.

/// How loud a source `distance` away is, given the `range` it carries.
///
/// Returns `1.0` at the listener and exactly `0.0` at `range`, falling off as
/// the square of what is left of the distance.
///
/// **Why not the inverse-distance law Unity and Godot default to.** Theirs is
/// physically right and never reaches zero, so every source in the scene stays
/// in the mix forever and both engines bolt a `maxDistance` cut-off on top —
/// which puts a step in the curve exactly where a sound is quietest and most
/// likely to be moving in and out. A curve that arrives at zero on its own has
/// no cut-off to hear, and near the listener it is within a few decibels of
/// the inverse law anyway. A tilemap of ambient sources is what makes this
/// matter, and that is stage 25.
///
/// A `range` of zero or less means "audible everywhere at full volume": the
/// author has said the sound is not positional, and a division that would
/// produce an infinity is not a better answer than the one they asked for.
pub fn attenuation(distance: f32, range: f32) -> f32 {
    if !range.is_finite() || range <= 0.0 {
        return 1.0;
    }
    if !distance.is_finite() {
        return 0.0;
    }

    let left = 1.0 - (distance.abs() / range).clamp(0.0, 1.0);
    left * left
}

/// Where a source sits between the ears, from its horizontal offset.
///
/// `offset` is the source's x minus the listener's x, in world units;
/// the result is [`PlayParams::pan`](crate::PlayParams::pan)'s `-1.0` to
/// `1.0`. A source a whole `range` to the right is hard right, and everything
/// between is linear in distance.
///
/// Only x. This is a 2D engine and both ears are on the same horizontal line:
/// a sound above the listener and one below it reach both ears equally, and
/// pretending otherwise would be inventing a head model for an axis that
/// carries no interaural difference. Height is what [`attenuation`] already
/// accounts for, through the distance.
pub fn pan(offset: f32, range: f32) -> f32 {
    if !offset.is_finite() || !range.is_finite() || range <= 0.0 {
        return 0.0;
    }
    (offset / range).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_source_at_the_listener_is_at_full_volume() {
        assert_eq!(attenuation(0.0, 10.0), 1.0);
    }

    #[test]
    fn a_source_at_the_edge_of_its_range_is_silent() {
        // Exactly zero, not nearly: this is what keeps a distant source out of
        // the mix without a separate cut-off.
        assert_eq!(attenuation(10.0, 10.0), 0.0);
        assert_eq!(attenuation(100.0, 10.0), 0.0);
    }

    #[test]
    fn the_falloff_is_monotonic() {
        let mut previous = f32::INFINITY;
        for step in 0..=20 {
            let gain = attenuation(step as f32 * 0.5, 10.0);
            assert!(gain <= previous, "gain rose at {step}");
            previous = gain;
        }
    }

    #[test]
    fn half_way_out_is_a_quarter_of_the_volume() {
        // The square is the whole shape of the curve, so it is worth pinning.
        assert!((attenuation(5.0, 10.0) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn distance_has_no_side() {
        // Attenuation is about how far, never which way; the pan carries that.
        assert_eq!(attenuation(-3.0, 10.0), attenuation(3.0, 10.0));
    }

    #[test]
    fn a_range_of_zero_means_the_sound_is_not_positional() {
        assert_eq!(attenuation(1_000.0, 0.0), 1.0);
        assert_eq!(attenuation(1_000.0, -1.0), 1.0);
        assert_eq!(pan(1_000.0, 0.0), 0.0);
    }

    #[test]
    fn a_source_on_the_listener_is_centred() {
        assert_eq!(pan(0.0, 10.0), 0.0);
    }

    #[test]
    fn a_source_to_the_right_pans_right() {
        assert!((pan(5.0, 10.0) - 0.5).abs() < 1e-6);
        assert_eq!(pan(10.0, 10.0), 1.0);
        assert_eq!(pan(40.0, 10.0), 1.0, "and no further");
    }

    #[test]
    fn a_source_to_the_left_pans_left() {
        assert!((pan(-5.0, 10.0) + 0.5).abs() < 1e-6);
        assert_eq!(pan(-40.0, 10.0), -1.0);
    }

    #[test]
    fn a_nan_offset_is_centred_and_silent_rather_than_poisoning_the_mix() {
        assert_eq!(pan(f32::NAN, 10.0), 0.0);
        assert_eq!(attenuation(f32::NAN, 10.0), 0.0);
        assert_eq!(attenuation(1.0, f32::NAN), 1.0);
    }
}
