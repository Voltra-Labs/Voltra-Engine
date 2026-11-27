//! Where in a [`SerialTree`] a value sits, where two trees part, and
//! reading and writing at a place.

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

/// Where `a` and `b` differ: the deepest places at which their shapes
/// still agree and their values do not. Empty when they are equal.
///
/// A struct (or a variant of the same name) is walked into field by
/// field, by name, and a map key by key: a field or an entry one side
/// has and the other does not is a place of its own (see
/// [`Found::Absent`]). A sequence or tuple of the same length is walked
/// element by element; anything else that differs is a place of its own
/// — a variant changed, a list grown or shrunk (so the whole list), two
/// numbers. So a changed `half_extents.x` answers that one float, and a
/// box turned into a circle answers the collider itself.
pub fn differences(a: &SerialTree, b: &SerialTree) -> Vec<TreePath> {
    let mut out = Vec::new();
    walk(a, b, &mut Vec::new(), &mut out);
    out
}

fn walk(a: &SerialTree, b: &SerialTree, path: &mut TreePath, out: &mut Vec<TreePath>) {
    if a == b {
        return;
    }
    match (a, b) {
        (T::Struct { name: n, fields: x }, T::Struct { name: m, fields: y }) if n == m => {
            fields(x, y, path, out);
        }
        (
            T::StructVariant {
                variant: v,
                fields: x,
                ..
            },
            T::StructVariant {
                variant: w,
                fields: y,
                ..
            },
        ) if v == w => {
            path.push(Step::Variant(v));
            fields(x, y, path, out);
            path.pop();
        }
        (
            T::TupleVariant {
                variant: v,
                fields: x,
                ..
            },
            T::TupleVariant {
                variant: w,
                fields: y,
                ..
            },
        ) if v == w && x.len() == y.len() => {
            path.push(Step::Variant(v));
            elements(x, y, path, out);
            path.pop();
        }
        (
            T::NewtypeVariant {
                variant: v,
                value: x,
                ..
            },
            T::NewtypeVariant {
                variant: w,
                value: y,
                ..
            },
        ) if v == w => {
            path.push(Step::Variant(v));
            walk(x, y, path, out);
            path.pop();
        }
        (T::NewtypeStruct { name: n, value: x }, T::NewtypeStruct { name: m, value: y })
            if n == m =>
        {
            path.push(Step::Inner);
            walk(x, y, path, out);
            path.pop();
        }
        (T::Some(x), T::Some(y)) => {
            path.push(Step::Inner);
            walk(x, y, path, out);
            path.pop();
        }
        (T::Seq(x), T::Seq(y)) | (T::Tuple(x), T::Tuple(y)) if x.len() == y.len() => {
            elements(x, y, path, out);
        }
        (T::TupleStruct { name: n, fields: x }, T::TupleStruct { name: m, fields: y })
            if n == m && x.len() == y.len() =>
        {
            elements(x, y, path, out);
        }
        (T::Map(x), T::Map(y)) => {
            for key in union(x.iter().map(|(k, _)| k), y.iter().map(|(k, _)| k)) {
                path.push(Step::Key(key.clone()));
                match (entry(x, key), entry(y, key)) {
                    (Some(x), Some(y)) => walk(x, y, path, out),
                    // On one side only: the entry is the place.
                    _ => out.push(path.clone()),
                }
                path.pop();
            }
        }
        _ => out.push(path.clone()),
    }
}

/// Each key of `x`, in order, then each of `y` that `x` lacks.
fn union<'k, K: PartialEq + 'k>(
    x: impl Iterator<Item = &'k K> + Clone,
    y: impl Iterator<Item = &'k K>,
) -> Vec<&'k K> {
    let mut keys: Vec<&K> = x.collect();
    for key in y {
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    keys
}

/// The value under `key` in a list of entries.
fn entry<'a, K: PartialEq>(entries: &'a [(K, SerialTree)], key: &K) -> Option<&'a SerialTree> {
    entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// A struct's fields walked by name, over both sides' names: a field
/// `skip_serializing_if` left out of one side is a place of its own, so a
/// sprite on its default sorting layer differs from one on another by
/// that field alone, not as a whole.
fn fields(
    x: &[(&'static str, SerialTree)],
    y: &[(&'static str, SerialTree)],
    path: &mut TreePath,
    out: &mut Vec<TreePath>,
) {
    for &key in union(x.iter().map(|(k, _)| k), y.iter().map(|(k, _)| k)) {
        path.push(Step::Field(key));
        match (entry(x, &key), entry(y, &key)) {
            (Some(x), Some(y)) => walk(x, y, path, out),
            _ => out.push(path.clone()),
        }
        path.pop();
    }
}

fn elements(x: &[SerialTree], y: &[SerialTree], path: &mut TreePath, out: &mut Vec<TreePath>) {
    for (i, (x, y)) in x.iter().zip(y).enumerate() {
        path.push(Step::Index(i));
        walk(x, y, path, out);
        path.pop();
    }
}

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
