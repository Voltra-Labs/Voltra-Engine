//! Where two [`SerialTree`]s part: the deepest places their shapes still
//! agree and their values do not.

use super::path::{Step, TreePath};
use super::SerialTree;
use SerialTree as T;

/// Where `a` and `b` differ: the deepest places at which their shapes
/// still agree and their values do not. Empty when they are equal.
///
/// A struct (or a variant of the same name) is walked into field by
/// field, by name, and a map key by key: a field or an entry one side
/// has and the other does not is a place of its own (see
/// [`Found::Absent`](super::Found::Absent)). A sequence or tuple of the same length is walked
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
///
/// A map left out on one side reads as an empty one and is walked key by
/// key: a map is left out when it is empty (`BTreeMap::is_empty`, the
/// only skip a map field has), so a tilemap gaining its first tile data,
/// or losing its last, differs by that tile's key and not by the whole
/// map. serde hands a serializer nothing of a skipped field but its name,
/// so this is what can be known of one without its type; any other field
/// left out is one place, written whole or left out whole.
fn fields(
    x: &[(&'static str, SerialTree)],
    y: &[(&'static str, SerialTree)],
    path: &mut TreePath,
    out: &mut Vec<TreePath>,
) {
    let empty = SerialTree::Map(Vec::new());
    for &key in union(x.iter().map(|(k, _)| k), y.iter().map(|(k, _)| k)) {
        path.push(Step::Field(key));
        match (entry(x, &key), entry(y, &key)) {
            (Some(x), Some(y)) => walk(x, y, path, out),
            (Some(map @ T::Map(_)), None) => walk(map, &empty, path, out),
            (None, Some(map @ T::Map(_))) => walk(&empty, map, path, out),
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
