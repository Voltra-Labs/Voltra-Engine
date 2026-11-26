//! A layer of a player: a second motion played over its own, masked to some
//! of its targets, at a weight, overriding or adding.

use serde::{Deserialize, Serialize};

use super::player::AnimationPlayer;
use super::target::canonical;

/// How a layer's values go over what is under it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LayerBlend {
    /// Replaces what is under it, by its weight: `lerp(under, layer, w)`,
    /// each property mixed as its [`Mixing`](super::Mixing) says — an angle
    /// the short way, a frame switching at `w >= 0.5`.
    #[default]
    Override,
    /// Adds its clips' change from their first frame, times its weight:
    /// `under + w * (value - value at 0)`. Numbers only; a discrete property
    /// (a frame) in an additive layer overrides.
    Additive,
}

/// One layer a player plays over its own motion — Unity's animator layer,
/// Godot's blend node over a filtered track set.
///
/// [`player`](Self::player) is what it plays: its own clip or synced clips,
/// clock, speed and fade, driven exactly as a player is (`play`,
/// `cross_fade`, …). Its own layers are not played. A layer that has ended,
/// or is paused, holds its pose over what is under it, as a layer should —
/// a squash played once stays on its last frame rather than vanishing.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerLayer {
    /// What the layer plays.
    pub player: AnimationPlayer,
    /// How much of it shows, `0..=1`; outside that it is clamped, and a
    /// weight that is not a number is zero. At zero the layer writes
    /// nothing.
    pub weight: f32,
    /// The targets it may write — each the path of an entity under the
    /// player, and everything under that entity: `"arm"` is the arm and
    /// its hand. Empty is every target.
    pub mask: Vec<String>,
    /// Whether it replaces what is under it or adds to it.
    pub blend: LayerBlend,
}

impl Default for PlayerLayer {
    fn default() -> Self {
        Self {
            player: AnimationPlayer::default(),
            weight: 1.0,
            mask: Vec::new(),
            blend: LayerBlend::Override,
        }
    }
}

impl PlayerLayer {
    /// A layer blending as `blend`, at full weight, unmasked, playing nothing.
    pub fn new(blend: LayerBlend) -> Self {
        Self {
            blend,
            ..Self::default()
        }
    }

    /// The weight it is mixed at: [`weight`](Self::weight) clamped to
    /// `0..=1`, zero for a weight that is not a number.
    pub fn effective_weight(&self) -> f32 {
        if self.weight.is_nan() {
            0.0
        } else {
            self.weight.clamp(0.0, 1.0)
        }
    }

    /// Whether its mask lets it write `target` (a path in
    /// [`canonical`] spelling).
    pub fn covers(&self, target: &str) -> bool {
        self.mask.is_empty() || self.mask.iter().any(|m| under(target, &canonical(m)))
    }
}

/// Whether `target` is `path` or under it; `""` is everything.
fn under(target: &str, path: &str) -> bool {
    path.is_empty()
        || target
            .strip_prefix(path)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mask_covers_its_paths_and_what_is_under_them() {
        let mut layer = PlayerLayer::default();
        assert!(layer.covers("anything"), "empty is every target");
        layer.mask = vec!["/arm/".to_owned()];
        assert!(layer.covers("arm") && layer.covers("arm/hand"));
        assert!(!layer.covers("armour") && !layer.covers("") && !layer.covers("leg"));
        layer.mask.push(String::new());
        assert!(
            layer.covers(""),
            "an empty path is the root and all under it"
        );
    }

    #[test]
    fn a_weight_is_clamped_and_not_a_number_is_zero() {
        let mut layer = PlayerLayer::default();
        for (weight, effective) in [(1.5, 1.0), (-1.0, 0.0), (f32::NAN, 0.0), (0.25, 0.25)] {
            layer.weight = weight;
            assert_eq!(layer.effective_weight(), effective);
        }
    }
}
