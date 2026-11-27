//! Where in a [`SerialTree`] a value sits, where two trees part, and
//! reading and writing at a place.

use super::SerialTree;

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
/// A struct (or a variant of the same name, a sequence of the same
/// length, a map with the same keys) is walked into; anything else that
/// differs is a place of its own — a variant changed, a list grown, two
/// numbers. So a changed `half_extents.x` answers that one float, and a
/// box turned into a circle answers the collider itself.
pub fn differences(a: &SerialTree, b: &SerialTree) -> Vec<TreePath> {
    let mut out = Vec::new();
    walk(a, b, &mut Vec::new(), &mut out);
    out
}

fn walk(a: &SerialTree, b: &SerialTree, path: &mut TreePath, out: &mut Vec<TreePath>) {
    use SerialTree as T;
    if a == b {
        return;
    }
    match (a, b) {
        (T::Struct { name: n, fields: x }, T::Struct { name: m, fields: y })
            if n == m && same_keys(x, y) =>
        {
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
        ) if v == w && same_keys(x, y) => {
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
        (T::Map(x), T::Map(y))
            if x.len() == y.len() && x.iter().zip(y).all(|((k, _), (l, _))| k == l) =>
        {
            for ((key, x), (_, y)) in x.iter().zip(y) {
                path.push(Step::Key(key.clone()));
                walk(x, y, path, out);
                path.pop();
            }
        }
        _ => out.push(path.clone()),
    }
}

fn same_keys(x: &[(&'static str, SerialTree)], y: &[(&'static str, SerialTree)]) -> bool {
    x.len() == y.len() && x.iter().zip(y).all(|((k, _), (l, _))| k == l)
}

fn fields(
    x: &[(&'static str, SerialTree)],
    y: &[(&'static str, SerialTree)],
    path: &mut TreePath,
    out: &mut Vec<TreePath>,
) {
    for ((key, x), (_, y)) in x.iter().zip(y) {
        path.push(Step::Field(key));
        walk(x, y, path, out);
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

impl SerialTree {
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
        use SerialTree as T;
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
        use SerialTree as T;
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
