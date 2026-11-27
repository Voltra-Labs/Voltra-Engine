//! Values sampled from clips and not yet written: what a blend mixes.

use std::borrow::Cow;

use voltra_ecs::{Entity, World};

use super::property::Property;
use super::target::{self, Children, Targets};
use super::value::Value;

/// One property's value on one entity under a player.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample<'a> {
    /// The entity, by names from the player down, as a track names it —
    /// in [`target::canonical`] spelling, so two clips naming one child
    /// two ways are one entry.
    pub target: Cow<'a, str>,
    /// Which property.
    pub property: Property,
    /// Its value.
    pub value: Value,
}

/// At most one value per (target, property), sampled from one clip or
/// mixed from several, not yet written to anything.
///
/// The buffer between sampling and writing: a clip samples into one
/// ([`AnimationClip::sample`](super::AnimationClip::sample)), a
/// [`blend`](super::blend::blend) mixes several into one, and
/// [`apply`](Self::apply) writes it once — so a property two inputs
/// animate is written once with their mix, never twice. Targets borrow
/// from the clip they were sampled from; [`into_owned`](Self::into_owned)
/// is a pose kept across frames, such as a fade's frozen start.
///
/// Kept sorted by target, then property: a lookup is a binary search, and
/// a blend of several poses walks them side by side, once each.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Pose<'a> {
    samples: Vec<Sample<'a>>,
}

impl<'a> Pose<'a> {
    /// A pose holding nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// A pose holding nothing, with room for `capacity` values.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            samples: Vec::with_capacity(capacity),
        }
    }

    /// Sets `property` of `target` to `value`, replacing a value already
    /// there — so the later of two tracks on one property is the one kept,
    /// as writing both in order would leave it.
    pub fn set(&mut self, target: impl Into<Cow<'a, str>>, property: Property, value: Value) {
        let target = match target.into() {
            Cow::Borrowed(path) => target::canonical(path),
            Cow::Owned(path) => Cow::Owned(target::canonical(&path).into_owned()),
        };
        // A clip's tracks are mostly in order already: one comparison then.
        let after_last = self
            .samples
            .last()
            .is_none_or(|last| (last.target.as_ref(), last.property) < (&*target, property));
        if after_last {
            self.samples.push(Sample {
                target,
                property,
                value,
            });
            return;
        }
        match self.find(&target, property) {
            Ok(at) => self.samples[at].value = value,
            Err(at) => self.samples.insert(
                at,
                Sample {
                    target,
                    property,
                    value,
                },
            ),
        }
    }

    /// The value it holds for `property` of `target`.
    pub fn get(&self, target: &str, property: Property) -> Option<Value> {
        let target = target::canonical(target);
        self.find(&target, property)
            .ok()
            .map(|at| self.samples[at].value)
    }

    /// Where (`target`, `property`) is, or would go; `target` canonical.
    fn find(&self, target: &str, property: Property) -> Result<usize, usize> {
        self.samples
            .binary_search_by(|s| (s.target.as_ref(), s.property).cmp(&(target, property)))
    }

    /// Appends a sample that sorts after every one held — what a blend,
    /// walking its inputs in order, produces. The caller keeps the order.
    pub(super) fn push_sorted(&mut self, sample: Sample<'a>) {
        debug_assert!(self.samples.last().is_none_or(|last| {
            (last.target.as_ref(), last.property) < (sample.target.as_ref(), sample.property)
        }));
        self.samples.push(sample);
    }

    /// Every value, to change in place. The caller changes values only: a
    /// target or property changed would break the order.
    pub(super) fn samples_mut(&mut self) -> &mut [Sample<'a>] {
        &mut self.samples
    }

    /// Keeps only the values `keep` answers true for — a layer's mask.
    pub(super) fn retain(&mut self, keep: impl FnMut(&Sample<'a>) -> bool) {
        self.samples.retain(keep);
    }

    /// Drops every value, keeping the room for the next frame's.
    pub fn clear(&mut self) {
        self.samples.clear();
    }

    /// Every value it holds, sorted by target, then property.
    pub fn samples(&self) -> &[Sample<'a>] {
        &self.samples
    }

    /// How many values it holds.
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Whether it holds nothing.
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// The same pose, owning its targets, to keep past the clips it was
    /// sampled from.
    pub fn into_owned(self) -> Pose<'static> {
        Pose {
            samples: self
                .samples
                .into_iter()
                .map(|s| Sample {
                    target: Cow::Owned(s.target.into_owned()),
                    property: s.property,
                    value: s.value,
                })
                .collect(),
        }
    }

    /// Becomes `other`, owning its targets: what a fade keeps of the mix it
    /// last wrote. Reuses the names it holds — a fade writes the same
    /// properties of the same targets frame after frame, so in the steady
    /// state this copies values and allocates nothing.
    pub fn copy_from(&mut self, other: &Pose<'_>) {
        self.samples.truncate(other.samples.len());
        for (mine, theirs) in self.samples.iter_mut().zip(&other.samples) {
            if mine.target != theirs.target {
                match &mut mine.target {
                    Cow::Owned(name) => {
                        name.clear();
                        name.push_str(&theirs.target);
                    }
                    borrowed => *borrowed = Cow::Owned(theirs.target.to_string()),
                }
            }
            mine.property = theirs.property;
            mine.value = theirs.value;
        }
        let have = self.samples.len();
        self.samples
            .extend(other.samples[have..].iter().map(|s| Sample {
                target: Cow::Owned(s.target.to_string()),
                property: s.property,
                value: s.value,
            }));
    }

    /// Writes every value onto the entity it names under `root`. Answers
    /// the values that found nothing to write to — a target no child is
    /// named, or one without the property's component; each costs only
    /// itself.
    pub fn apply(&self, world: &mut World, root: Entity) -> Vec<&Sample<'a>> {
        let children = Children::of(world);
        self.apply_through(world, root, &children)
    }

    /// [`apply`](Self::apply), finding targets through `children` — one
    /// index for every pose written in a frame.
    pub(super) fn apply_through(
        &self,
        world: &mut World,
        root: Entity,
        children: &Children,
    ) -> Vec<&Sample<'a>> {
        let mut targets = Targets::new(children);
        let mut missed = Vec::new();
        for sample in &self.samples {
            let written = targets
                .get(world, root, &sample.target)
                .is_some_and(|entity| sample.property.write(world, entity, sample.value));
            if !written {
                missed.push(sample);
            }
        }
        missed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_property_set_twice_holds_the_later_value_under_one_spelling() {
        let mut pose = Pose::new();
        pose.set("arm/hand", Property::Rotation, Value::Float(1.0));
        pose.set("/arm//hand/", Property::Rotation, Value::Float(2.0));
        pose.set(String::from("arm"), Property::Rotation, Value::Float(3.0));
        assert_eq!(pose.len(), 2);
        assert_eq!(
            pose.get("arm/hand", Property::Rotation),
            Some(Value::Float(2.0))
        );
        assert_eq!(pose.get("arm", Property::Scale), None);
        let owned = pose.clone().into_owned();
        assert_eq!(owned.samples(), pose.samples());
    }
}
