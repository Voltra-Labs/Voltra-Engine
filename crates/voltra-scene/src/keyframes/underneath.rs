//! What a player's layers cover: each property's value under them, and the
//! value they last wrote over it.

use std::borrow::Cow;

use voltra_ecs::{Entity, World};

use super::blend::Mixing;
use super::pose::Pose;
use super::property::Property;
use super::target::Children;
use super::value::Value;

/// One property a layer writes: what is under the layers, and what was
/// last written over it.
#[derive(Debug, Clone, PartialEq)]
struct Covered {
    target: String,
    property: Property,
    /// The value with no layer over it — the player's own motion's, or,
    /// for a property that motion does not animate, what the world held
    /// before the layers wrote it.
    under: Value,
    /// What the frame wrote, read back: a world still holding it has not
    /// been changed by anything else since.
    written: Value,
}

/// The properties a player's layers write, sorted by target and property.
///
/// A layer composites over what is under it. For a property the player's
/// own motion animates, that is the motion's value, fresh each frame. For
/// one it does not, it is the world's value — but the world holds what the
/// layer wrote last frame, and compositing over that would compound: an
/// override at `0.01` would creep, frame by frame, all the way to the
/// layer's value. So the value from before the layers is kept here, and
/// used while the world still holds what they wrote; once something else
/// writes the property (a game, the editor), the change it made is carried
/// under them too ([`moved`]). Unity keeps the scene's default values for
/// the same reason ("write defaults").
///
/// A property no layer writes any more — its weight fell to zero, its
/// state changed, the layer went — is handed back: what was under the
/// layers is written once, if nothing else has written it since. So a
/// layer cut to weight zero leaves nothing of itself behind.
#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct Underneath(Vec<Covered>);

impl Underneath {
    /// Whether no layer covers anything.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Fills `acc` — the player's own motion's pose — with what is under
    /// the layers for each of `keys` (sorted, unique) it does not hold, and
    /// hands back what no key covers any more. [`settle`](Self::settle)
    /// reads back what the frame wrote.
    pub fn fill<'a>(
        &mut self,
        world: &mut World,
        root: Entity,
        children: &Children,
        keys: &[(Cow<'a, str>, Property)],
        acc: &mut Pose<'a>,
    ) {
        let old = std::mem::take(&mut self.0);
        let mut kept = Vec::with_capacity(keys.len());
        let mut old_at = old.into_iter().peekable();
        let mut find = Resolver::new(children, root);
        for (target, property) in keys {
            let key = (target.as_ref(), *property);
            // Entries no key names any more, before this one: handed back.
            while let Some(gone) = old_at.next_if(|c| (c.target.as_str(), c.property) < key) {
                hand_back(world, &mut find, acc, gone);
            }
            let last = old_at.next_if(|c| (c.target.as_str(), c.property) == key);
            let held = acc.get(target, *property);
            let under = match (held, &last) {
                (Some(value), _) => Some(value),
                (None, last) => {
                    let now = find
                        .get(world, target)
                        .and_then(|e| property.read(world, e));
                    match (now, last) {
                        // Nothing else wrote it since: what was under stays.
                        (Some(now), Some(last)) if now == last.written => Some(last.under),
                        // Something did: what it changed goes under the
                        // layers too.
                        (Some(now), Some(last)) => Some(moved(*property, last, now)),
                        (now, _) => now,
                    }
                }
            };
            let Some(under) = under else {
                // Nothing to write to: the layer's value goes out whole and
                // is missed as the motion's would be.
                continue;
            };
            if held.is_none() {
                acc.set(target.clone(), *property, under);
            }
            // The name kept from last frame, when there is one: a layer
            // covers the same properties frame after frame.
            let target = match last {
                Some(last) => last.target,
                None => target.to_string(),
            };
            kept.push(Covered {
                target,
                property: *property,
                under,
                written: under,
            });
        }
        for gone in old_at {
            hand_back(world, &mut find, acc, gone);
        }
        self.0 = kept;
    }

    /// Reads back what the frame wrote over each property covered, for
    /// the next frame to tell whether anything else wrote it since.
    pub fn settle(&mut self, world: &World, root: Entity, children: &Children) {
        let mut find = Resolver::new(children, root);
        self.0.retain_mut(|c| {
            let now = find
                .get(world, &c.target)
                .and_then(|e| c.property.read(world, e));
            match now {
                Some(now) => {
                    c.written = now;
                    true
                }
                None => false,
            }
        });
    }
}

/// Targets resolved under one root, the last one remembered: the entries
/// are sorted, so a target's several properties resolve it once.
struct Resolver<'c> {
    children: &'c Children,
    root: Entity,
    last: String,
    found: Option<Option<Entity>>,
}

impl<'c> Resolver<'c> {
    fn new(children: &'c Children, root: Entity) -> Self {
        Self {
            children,
            root,
            last: String::new(),
            found: None,
        }
    }

    fn get(&mut self, world: &World, path: &str) -> Option<Entity> {
        match self.found {
            Some(found) if self.last == path => found,
            _ => {
                let found = self.children.resolve(world, self.root, path);
                self.last.clear();
                self.last.push_str(path);
                self.found = Some(found);
                found
            }
        }
    }
}

/// What is under the layers once something other than them wrote `now`
/// over what they last wrote.
///
/// For a number, the change it made goes under them: `under + (now -
/// written)`. A game moving an entity by a step each frame
/// (`translation += v * dt`) reads the value the layers wrote and adds to
/// it; taking `now` whole as what is under would put the layers' own
/// contribution under them again every frame — an additive bob would add
/// itself up without end, and an override below one would damp the game's
/// motion. A game that sets the value outright each frame converges, the
/// same way, on showing exactly what it sets. A discrete value (a frame)
/// has no difference: what was written is taken.
fn moved(property: Property, last: &Covered, now: Value) -> Value {
    if property.mixing() == Mixing::Discrete {
        return now;
    }
    match (last.under, now, last.written) {
        (Value::Float(u), Value::Float(n), Value::Float(w)) => Value::Float(u + (n - w)),
        (Value::Vec2(u), Value::Vec2(n), Value::Vec2(w)) => Value::Vec2(u + (n - w)),
        (Value::Color(u), Value::Color(n), Value::Color(w)) => {
            Value::Color(std::array::from_fn(|i| u[i] + (n[i] - w[i])))
        }
        _ => now,
    }
}

/// Writes back what was under the layers for a property they no longer
/// write — unless the motion writes it now, or something else wrote it
/// since the layers did.
fn hand_back(world: &mut World, find: &mut Resolver<'_>, acc: &Pose<'_>, gone: Covered) {
    if acc.get(&gone.target, gone.property).is_some() {
        return;
    }
    let Some(entity) = find.get(world, &gone.target) else {
        return;
    };
    if gone.property.read(world, entity) == Some(gone.written) {
        gone.property.write(world, entity, gone.under);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn covered(under: Value, written: Value, property: Property) -> Covered {
        Covered {
            target: String::new(),
            property,
            under,
            written,
        }
    }

    #[test]
    fn a_number_carries_the_change_under_and_a_frame_is_taken_whole() {
        let last = covered(Value::Float(1.0), Value::Float(1.5), Property::Rotation2D);
        assert_eq!(
            moved(Property::Rotation2D, &last, Value::Float(1.75)),
            Value::Float(1.25)
        );
        let last = covered(
            Value::Color([0.0, 0.0, 0.0, 1.0]),
            Value::Color([0.5, 0.5, 0.5, 1.0]),
            Property::SpriteColor,
        );
        assert_eq!(
            moved(
                Property::SpriteColor,
                &last,
                Value::Color([1.0, 0.5, 0.5, 1.0])
            ),
            Value::Color([0.5, 0.0, 0.0, 1.0])
        );
        let last = covered(Value::Int(2), Value::Int(5), Property::SpriteFrame);
        assert_eq!(
            moved(Property::SpriteFrame, &last, Value::Int(7)),
            Value::Int(7)
        );
    }
}
