//! One property of one entity, keyed over time.

use serde::{Deserialize, Serialize};

use super::ease::Ease;
use super::property::Property;
use super::value::Value;

/// A value at a time.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Key {
    /// Seconds from the clip's start.
    pub time: f32,
    /// What the property is at `time`.
    pub value: Value,
    /// The shape of the segment from this key to the next. Linear when not
    /// written.
    #[serde(default, skip_serializing_if = "Ease::is_linear")]
    pub ease: Ease,
}

impl Key {
    /// A key at `time` holding `value`, eased linearly.
    pub fn new(time: f32, value: Value) -> Self {
        Self {
            time,
            value,
            ease: Ease::Linear,
        }
    }

    /// Sets the shape of the segment from this key to the next.
    pub fn with_ease(mut self, ease: Ease) -> Self {
        self.ease = ease;
        self
    }
}

/// The keys of one property on one entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    /// The entity, by names from the player down — see
    /// [`resolve`](super::target::resolve). Empty, and not written, is the
    /// player's own.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub target: String,
    /// Which property it drives.
    pub property: Property,
    /// Sorted by time; a clip's load sorts them.
    #[serde(default)]
    pub keys: Vec<Key>,
}

/// Two keys closer than this are one: what an editor's "key here" replaces
/// rather than adds beside.
pub const SAME_TIME: f32 = 1e-4;

impl Track {
    /// A track on `target` with no keys.
    pub fn new(target: impl Into<String>, property: Property) -> Self {
        Self {
            target: target.into(),
            property,
            keys: Vec::new(),
        }
    }

    /// The value at `time`: the first key's before it, the last key's after
    /// it, and between two keys the earlier one's ease. Two keys at one time
    /// cut — the later is held from then on. `None` for a track with no
    /// keys.
    pub fn sample(&self, time: f32) -> Option<Value> {
        let first = self.keys.first()?;
        let next = self.keys.partition_point(|k| k.time <= time);
        if next == 0 {
            return Some(first.value);
        }
        let from = &self.keys[next - 1];
        let Some(to) = self.keys.get(next) else {
            return Some(from.value);
        };
        // `from.time <= time < to.time`, so the span is positive.
        let t = (time - from.time) / (to.time - from.time);
        Some(from.value.lerp(to.value, from.ease.apply(t)))
    }

    /// Keys `value` at `time`, replacing a key already there (keeping its
    /// ease) or inserting in order. Answers the key's index.
    pub fn set_key(&mut self, time: f32, value: Value) -> usize {
        if let Some(at) = self.key_at(time) {
            self.keys[at].value = value;
            return at;
        }
        let at = self.keys.partition_point(|k| k.time < time);
        self.keys.insert(at, Key::new(time, value));
        at
    }

    /// The key within [`SAME_TIME`] of `time`, if there is one.
    pub fn key_at(&self, time: f32) -> Option<usize> {
        self.keys
            .iter()
            .position(|k| (k.time - time).abs() < SAME_TIME)
    }

    /// The time of the last key, or zero.
    pub fn end(&self) -> f32 {
        self.keys.last().map_or(0.0, |k| k.time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fade() -> Track {
        let mut track = Track::new("", Property::PointLight2DIntensity);
        track.set_key(1.0, Value::Float(2.0));
        track.set_key(0.0, Value::Float(0.0));
        track
    }

    #[test]
    fn a_track_holds_its_ends_and_eases_between() {
        let mut track = fade();
        assert_eq!(track.sample(-1.0), Some(Value::Float(0.0)));
        assert_eq!(track.sample(0.5), Some(Value::Float(1.0)));
        assert_eq!(track.sample(9.0), Some(Value::Float(2.0)));
        track.keys[0].ease = Ease::Step;
        assert_eq!(track.sample(0.99), Some(Value::Float(0.0)));
        assert_eq!(track.sample(1.0), Some(Value::Float(2.0)));
        assert_eq!(Track::new("", Property::Rotation2D).sample(0.0), None);
    }

    #[test]
    fn keying_a_time_already_keyed_replaces_the_value_and_keeps_the_ease() {
        let mut track = fade();
        track.keys[1].ease = Ease::EaseOut;
        assert_eq!(track.set_key(1.0 + SAME_TIME / 2.0, Value::Float(5.0)), 1);
        assert_eq!(track.keys.len(), 2);
        assert_eq!(track.keys[1].ease, Ease::EaseOut);
        assert_eq!(track.set_key(0.5, Value::Float(1.0)), 1, "in order");
        assert_eq!(track.end(), 1.0);
    }
}
