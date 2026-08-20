//! What a collider interacts with, and whether it is solid.

use serde::{Deserialize, Serialize};

/// Which layers a collider is on, and which layers it looks at.
///
/// Two bitmasks per collider, as Box2D's `b2Filter`, Godot's
/// `collision_layer`/`collision_mask` and Rapier's `InteractionGroups` all
/// have. Unity's alternative — thirty-two named layers and one global
/// collision matrix — is a project settings file, and there is no project
/// settings file to put it in; a pair of `u32`s serialises next to the
/// collider it belongs to and needs no global anything.
///
/// The rule is *symmetric*: a pair interacts only if each side is on a layer
/// the other is looking at. Godot's one-sided test — where A detects B while B
/// walks straight through A — is the source of its most reported confusion.
/// Two colliders either interact or they do not.
///
/// An entity without this component interacts with everything, so no scene
/// written before it existed changes behaviour by loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollisionLayers {
    /// The layers this collider is *on* — what the other side's mask tests.
    pub layers: u32,
    /// The layers this collider *looks at*.
    pub mask: u32,
}

impl Default for CollisionLayers {
    fn default() -> Self {
        Self {
            layers: Self::ALL,
            mask: Self::ALL,
        }
    }
}

impl CollisionLayers {
    /// Every layer at once, which is what a collider that has never been
    /// filtered is on and looks at.
    pub const ALL: u32 = u32::MAX;

    /// How many layers a mask holds. The width of the mask, not a policy: a
    /// thirty-third layer needs a wider integer, not a bigger constant.
    pub const COUNT: u32 = u32::BITS;

    /// The mask of the single layer `index`, or nothing at all past the last
    /// layer.
    ///
    /// Zero rather than a wrapped bit for an out-of-range index: a scene file
    /// is external input, and layer 32 quietly meaning layer 0 puts a collider
    /// on a layer nobody wrote down.
    pub const fn bit(index: u32) -> u32 {
        if index < Self::COUNT {
            1 << index
        } else {
            0
        }
    }

    /// On `layers`, looking at `mask`.
    pub const fn new(layers: u32, mask: u32) -> Self {
        Self { layers, mask }
    }

    /// On the single layer `index`, looking at everything.
    pub const fn on(index: u32) -> Self {
        Self::new(Self::bit(index), Self::ALL)
    }

    /// Looking at the single layer `index`, on every layer.
    pub const fn looking_at(mut self, index: u32) -> Self {
        self.mask = Self::bit(index);
        self
    }

    /// Whether this collider looks at anything `other` is on.
    ///
    /// Half of the answer. [`CollisionLayers::interact`] is the whole one, and
    /// is what callers want.
    pub const fn sees(&self, other: &Self) -> bool {
        self.mask & other.layers != 0
    }

    /// Whether two colliders interact at all, missing components included.
    ///
    /// A collider with no filter is the default one, so an unfiltered pair
    /// interacts and an entity nobody has thought about still collides.
    pub fn interact(a: Option<&Self>, b: Option<&Self>) -> bool {
        let default = Self::default();
        let a = a.unwrap_or(&default);
        let b = b.unwrap_or(&default);
        a.sees(b) && b.sees(a)
    }
}

/// Detected, never solved.
///
/// A collider carrying this reports what overlaps it and stops there: no
/// constraint, no impulse, nothing in the solver's contacts. Unity's
/// `isTrigger` and Box2D's sensor fixture, as a marker component rather than a
/// flag — [`Collider`] is an enum, and a flag on it would be repeated in every
/// variant and in every shape added later. Godot's answer, a separate `Area2D`
/// node, has no equivalent here: this engine has components, not nodes.
///
/// A sensor still needs a [`Collider`] to have a shape, and its
/// [`CollisionLayers`] still decide what it can detect.
///
/// [`Collider`]: crate::Collider
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Sensor;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_collider_with_no_filter_interacts_with_everything() {
        assert!(CollisionLayers::interact(None, None));
        assert!(CollisionLayers::interact(
            Some(&CollisionLayers::on(3)),
            None
        ));
    }

    #[test]
    fn a_pair_interacts_when_each_side_looks_at_the_other() {
        let player = CollisionLayers::new(CollisionLayers::bit(0), CollisionLayers::bit(1));
        let ground = CollisionLayers::new(CollisionLayers::bit(1), CollisionLayers::bit(0));

        assert!(CollisionLayers::interact(Some(&player), Some(&ground)));
    }

    #[test]
    fn one_side_looking_away_is_enough_to_separate_them() {
        // The asymmetric alternative would have the player detect the ghost
        // while the ghost passed through the player. Either they interact or
        // they do not.
        let player = CollisionLayers::new(CollisionLayers::bit(0), CollisionLayers::ALL);
        let ghost = CollisionLayers::new(CollisionLayers::bit(1), CollisionLayers::bit(1));

        assert!(player.sees(&ghost), "the player looks at every layer");
        assert!(!ghost.sees(&player));
        assert!(!CollisionLayers::interact(Some(&player), Some(&ghost)));
        assert!(!CollisionLayers::interact(Some(&ghost), Some(&player)));
    }

    #[test]
    fn a_collider_on_no_layer_at_all_interacts_with_nothing() {
        let disabled = CollisionLayers::new(0, 0);

        assert!(!CollisionLayers::interact(Some(&disabled), None));
        assert!(!CollisionLayers::interact(
            Some(&disabled),
            Some(&CollisionLayers::default())
        ));
    }

    #[test]
    fn a_layer_past_the_last_one_is_not_a_layer() {
        assert_eq!(CollisionLayers::bit(31), 1 << 31);
        assert_eq!(CollisionLayers::bit(32), 0, "and not layer 0 again");
        assert_eq!(CollisionLayers::bit(u32::MAX), 0);
    }

    #[test]
    fn looking_at_one_layer_leaves_the_layers_alone() {
        let filter = CollisionLayers::on(2).looking_at(5);

        assert_eq!(filter.layers, CollisionLayers::bit(2));
        assert_eq!(filter.mask, CollisionLayers::bit(5));
    }

    #[test]
    fn a_filter_and_a_sensor_round_trip_through_ron() {
        let filter = CollisionLayers::on(4).looking_at(7);
        let text = ron::to_string(&filter).expect("serialise");
        let back: CollisionLayers = ron::from_str(&text).expect("deserialise");
        assert_eq!(back, filter);

        let text = ron::to_string(&Sensor).expect("serialise");
        let back: Sensor = ron::from_str(&text).expect("deserialise");
        assert_eq!(back, Sensor);
    }
}
