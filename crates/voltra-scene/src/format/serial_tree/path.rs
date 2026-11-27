//! Where in a [`SerialTree`] a value sits, and reading and writing at a
//! place.

use super::SerialTree;
use SerialTree as T;

/// One step down a [`SerialTree`].
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// A struct's (or struct variant's) field, by name.
    Field(&'static str),
    /// An element of a sequence, tuple, tuple struct or tuple variant.
    Index(usize),
    /// Into the enum variant of this name; matches no other variant, so a
    /// field two variants both call `radius` is never confused.
    Variant(&'static str),
    /// Into a `Some`, or a newtype struct's value.
    Inner,
    /// A map's value under this key.
    Key(SerialTree),
}

/// A place in a [`SerialTree`], from its root: Unity's property path.
pub type TreePath = Vec<Step>;

/// Sets, adds or removes the entry `key` in `entries`; whether it changed.
fn put_entry<K: PartialEq + Clone>(
    entries: &mut Vec<(K, SerialTree)>,
    key: &K,
    value: Option<&SerialTree>,
) -> bool {
    let at = entries.iter().position(|(k, _)| k == key);
    match (at, value) {
        (Some(i), Some(value)) if entries[i].1 != *value => {
            entries[i].1 = value.clone();
            true
        }
        (Some(_), Some(_)) => false,
        (Some(i), None) => {
            entries.remove(i);
            true
        }
        (None, Some(value)) => {
            entries.push((key.clone(), value.clone()));
            true
        }
        (None, None) => false,
    }
}

/// `path` as a person reads it: `shape.half_extents[0]`. Variants and
/// `Some`s are passed through without a word; the root is `""`.
pub fn describe(path: &[Step]) -> String {
    let mut text = String::new();
    for step in path {
        match step {
            Step::Field(name) => {
                if !text.is_empty() {
                    text.push('.');
                }
                text.push_str(name);
            }
            Step::Index(i) => text.push_str(&format!("[{i}]")),
            Step::Key(_) => text.push_str("[…]"),
            Step::Variant(_) | Step::Inner => {}
        }
    }
    text
}

/// What [`SerialTree::find`] found at a place.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Found<'a> {
    /// The value there.
    Here(&'a SerialTree),
    /// A field the struct there does not write, or a key its map does not
    /// hold: a value the type leaves at its default, a place that exists.
    Absent,
    /// No such place: another variant on the way, an index past the end,
    /// a step into something that is not a struct.
    Nowhere,
}

impl SerialTree {
    /// What is at `path`: a value, a field or key left out, or no such
    /// place.
    pub fn find(&self, path: &[Step]) -> Found<'_> {
        let Some((last, up)) = path.split_last() else {
            return Found::Here(self);
        };
        let Some(parent) = self.at(up) else {
            return Found::Nowhere;
        };
        match parent.child(last) {
            Some(value) => Found::Here(value),
            None if parent.holds_keyed(last) => Found::Absent,
            None => Found::Nowhere,
        }
    }

    /// Makes `path` hold `value` — `None` to leave the field or key out,
    /// as the type does for a value it skips, so its default comes back
    /// when it is read. A field or key missing here is added. Answers
    /// whether the tree changed; a tree without the place is left alone.
    pub fn put(&mut self, path: &[Step], value: Option<&SerialTree>) -> bool {
        let Some((last, up)) = path.split_last() else {
            return match value {
                Some(value) if self != value => {
                    *self = value.clone();
                    true
                }
                _ => false,
            };
        };
        let Some(parent) = self.at_mut(up) else {
            return false;
        };
        match (last, parent, value) {
            (
                Step::Field(key),
                T::Struct { fields, .. } | T::StructVariant { fields, .. },
                value,
            ) => put_entry(fields, key, value),
            (Step::Key(key), T::Map(entries), value) => put_entry(entries, key, value),
            (step, parent, Some(value)) => match parent.child_mut(step) {
                Some(slot) if slot != value => {
                    *slot = value.clone();
                    true
                }
                _ => false,
            },
            (_, _, None) => false,
        }
    }

    /// Whether this node is a struct (for a field step) or a map (for a
    /// key step) — where a missing child is absent rather than nowhere.
    fn holds_keyed(&self, step: &Step) -> bool {
        matches!(
            (step, self),
            (Step::Field(_), T::Struct { .. } | T::StructVariant { .. })
                | (Step::Key(_), T::Map(_))
        )
    }

    /// The value at `path`, or `None` where this tree has no such place —
    /// another variant there, a shorter list, a missing key.
    pub fn at(&self, path: &[Step]) -> Option<&SerialTree> {
        path.iter().try_fold(self, |node, step| node.child(step))
    }

    /// The value at `path`, to write.
    pub fn at_mut(&mut self, path: &[Step]) -> Option<&mut SerialTree> {
        path.iter()
            .try_fold(self, |node, step| node.child_mut(step))
    }

    /// Puts `value` at `path`, answering whether this tree has the place.
    /// A tree without it is left as it was.
    pub fn set(&mut self, path: &[Step], value: SerialTree) -> bool {
        match self.at_mut(path) {
            Some(slot) => {
                *slot = value;
                true
            }
            None => false,
        }
    }

    fn child(&self, step: &Step) -> Option<&SerialTree> {
        match (step, self) {
            (Step::Field(key), T::Struct { fields, .. } | T::StructVariant { fields, .. }) => {
                fields.iter().find(|(k, _)| k == key).map(|(_, v)| v)
            }
            (
                Step::Index(i),
                T::Seq(fields)
                | T::Tuple(fields)
                | T::TupleStruct { fields, .. }
                | T::TupleVariant { fields, .. },
            ) => fields.get(*i),
            (
                Step::Variant(want),
                T::UnitVariant { variant, .. }
                | T::StructVariant { variant, .. }
                | T::TupleVariant { variant, .. },
            ) if variant == want => Some(self),
            (Step::Variant(want), T::NewtypeVariant { variant, value, .. }) if variant == want => {
                Some(value)
            }
            (Step::Inner, T::Some(value) | T::NewtypeStruct { value, .. }) => Some(value),
            (Step::Key(key), T::Map(entries)) => {
                entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
            }
            _ => None,
        }
    }

    fn child_mut(&mut self, step: &Step) -> Option<&mut SerialTree> {
        // Decided before the match: a variant step answers the node itself,
        // which a match on `&mut self` cannot hand back.
        let own = match &*self {
            T::UnitVariant { variant, .. }
            | T::StructVariant { variant, .. }
            | T::TupleVariant { variant, .. } => Some(*variant),
            _ => None,
        };
        if let (Step::Variant(want), Some(variant)) = (step, own) {
            return (variant == *want).then_some(self);
        }
        match (step, self) {
            (Step::Field(key), T::Struct { fields, .. } | T::StructVariant { fields, .. }) => {
                fields.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v)
            }
            (
                Step::Index(i),
                T::Seq(fields)
                | T::Tuple(fields)
                | T::TupleStruct { fields, .. }
                | T::TupleVariant { fields, .. },
            ) => fields.get_mut(*i),
            (Step::Variant(want), T::NewtypeVariant { variant, value, .. }) => {
                (*variant == *want).then_some(value.as_mut())
            }
            (Step::Inner, T::Some(value) | T::NewtypeStruct { value, .. }) => Some(value),
            (Step::Key(key), T::Map(entries)) => {
                entries.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v)
            }
            _ => None,
        }
    }
}
