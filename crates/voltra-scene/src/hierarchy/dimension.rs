//! Which dimension an entity is placed in, and the rule that a hierarchy
//! keeps to one.
//!
//! An entity is 2D when it carries a [`Transform2D`], 3D when it carries a
//! [`Transform3D`], and neither when it carries no transform — a grouping
//! node, which passes its children through. A chain composes only its own
//! dimension's transforms (`compose.rs`), so a 2D child under a 3D
//! parent (or the reverse) has no meaning to compose. Godot's two trees,
//! `Node2D` and `Node3D`, keep the same rule by type; ours is kept at the
//! only writes that can break it: [`super::set_parent`] and adding a
//! transform ([`can_take_transform`]).
//!
//! A file can break it anyway — a hand edit, a merge. Such a file is warned
//! about on load ([`problems`], from [`super::resolve_parents`]) and left
//! alone: dropping a link or a transform would be an edit to someone's scene
//! made without asking, as for a parented physics body.

use std::collections::HashSet;
use std::sync::Mutex;

use voltra_ecs::{Entity, World};

use super::link::HierarchyError;
use super::walk::{ancestors, children_of, parent_of};
use crate::scene_id::SceneId;
use crate::transform::Transform2D;
use crate::transform3d::Transform3D;

/// One of the two parallel subsystems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dimension {
    /// Placed by a [`Transform2D`].
    D2,
    /// Placed by a [`Transform3D`].
    D3,
}

impl Dimension {
    /// The registered name of this dimension's transform.
    pub fn transform_name(self) -> &'static str {
        match self {
            Self::D2 => Transform2D::NAME,
            Self::D3 => Transform3D::NAME,
        }
    }

    /// The dimension whose transform is registered as `name`, if it is one.
    pub fn of_transform_name(name: &str) -> Option<Self> {
        [Self::D2, Self::D3]
            .into_iter()
            .find(|dimension| dimension.transform_name() == name)
    }

    /// The other one.
    pub fn other(self) -> Self {
        match self {
            Self::D2 => Self::D3,
            Self::D3 => Self::D2,
        }
    }

    /// Whether `entity` carries this dimension's transform.
    pub fn is_on(self, world: &World, entity: Entity) -> bool {
        match self {
            Self::D2 => world.get::<Transform2D>(entity).is_some(),
            Self::D3 => world.get::<Transform3D>(entity).is_some(),
        }
    }

    /// The dimensions `entity` carries a transform of: none, one, or — in a
    /// file that broke the rule — both.
    pub fn of(world: &World, entity: Entity) -> Vec<Self> {
        [Self::D2, Self::D3]
            .into_iter()
            .filter(|dimension| dimension.is_on(world, entity))
            .collect()
    }
}

/// What a loaded world holds against the rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    /// The entity carries both transforms.
    BothTransforms(Entity),
    /// The entity's transform is of one dimension and the nearest transform
    /// above it (`above`) of the other.
    MixedLink {
        /// The entity whose link crosses.
        child: Entity,
        /// The nearest ancestor with a transform.
        above: Entity,
    },
}

/// Every [`Problem`] in `world`, in index order: an entity is named once,
/// as the child of at most one problem.
///
/// An entity with both transforms is that problem and no other; a child
/// under one is not flagged as well — its parent's problem is the one to
/// fix, and every child of a broken entity reported again would bury it.
/// So a [`Problem::MixedLink`] is a child of exactly one dimension under a
/// nearest transform of exactly the other.
///
/// Free for a world with no [`Transform3D`] in it — every 2D game — which
/// cannot break the rule: one storage lookup and out.
pub fn problems(world: &World) -> Vec<Problem> {
    let Some(three) = world.components::<Transform3D>() else {
        return Vec::new();
    };
    let two = world.components::<Transform2D>();
    let placed = three
        .iter()
        .map(|(entity, _)| entity)
        .chain(two.into_iter().flat_map(|two| two.iter().map(|(e, _)| e)));
    let mut found = Vec::new();
    let mut seen = HashSet::new();
    for child in placed {
        if !seen.insert(child) {
            continue;
        }
        let ours = Dimension::of(world, child);
        if ours.len() > 1 {
            found.push(Problem::BothTransforms(child));
            continue;
        }
        let Some(parent) = parent_of(world, child) else {
            continue;
        };
        let Some((above, theirs)) = nearest_above(world, parent) else {
            continue;
        };
        if theirs.len() == 1 && theirs != ours {
            found.push(Problem::MixedLink { child, above });
        }
    }
    found.sort_unstable_by_key(|problem| match problem {
        Problem::BothTransforms(e) | Problem::MixedLink { child: e, .. } => e.index(),
    });
    found
}

/// Logs each of `world`'s [`problems`] the first time this run meets it.
///
/// Called by [`super::resolve_parents`], which runs after every load, every
/// record an undo puts back and every reparent: warning on each call would
/// say one broken link once per record of every undo. Remembered by the
/// entities' [`SceneId`]s, which an undo keeps, so the same link in the same
/// scene is said once; a transient entity has no id and is said once for
/// all of them.
/// Answers how many it warned about.
pub(super) fn warn_new_problems(world: &World) -> usize {
    static WARNED: Mutex<Option<HashSet<WarnedAbout>>> = Mutex::new(None);
    let found = problems(world);
    if found.is_empty() {
        return 0;
    }
    let id = |entity: Entity| world.get::<SceneId>(entity).copied();
    let Ok(mut warned) = WARNED.lock() else {
        return 0;
    };
    let mut said = 0;
    let warned = warned.get_or_insert_with(HashSet::new);
    for problem in found {
        let key = match problem {
            Problem::BothTransforms(entity) => (id(entity), None),
            Problem::MixedLink { child, above } => (id(child), Some(id(above))),
        };
        if !warned.insert(key) {
            continue;
        }
        said += 1;
        match problem {
            Problem::BothTransforms(entity) => log::warn!(
                "{entity:?} carries both Transform2D and Transform3D; each dimension draws it \
                 by its own, and the editor will not add either again"
            ),
            Problem::MixedLink { child, above } => log::warn!(
                "{child:?} is parented under {above:?} of the other dimension; it is composed \
                 as if that link were not there"
            ),
        }
    }
    said
}

/// What a warning was given about: the entity's id, and for a link the
/// id of the entity above it.
type WarnedAbout = (Option<SceneId>, Option<Option<SceneId>>);

/// Whether linking `child` under `parent` would compose one dimension
/// under the other: the nearest transform at or above `parent`, against
/// the nearest at or below `child` on every branch.
pub(super) fn link_mixes(world: &World, child: Entity, parent: Entity) -> bool {
    let Some((_, above)) = nearest_above(world, parent) else {
        return false;
    };
    let below = nearest_below(world, child);
    !below.is_empty() && spans_both(&above, &below)
}

/// Whether `a` and `b` between them hold both dimensions.
fn spans_both(a: &[Dimension], b: &[Dimension]) -> bool {
    let held = |d: Dimension| a.contains(&d) || b.contains(&d);
    held(Dimension::D2) && held(Dimension::D3)
}

/// Whether `entity` may be given `dimension`'s transform, and why not.
///
/// Refused when it already carries the other one
/// ([`HierarchyError::SecondTransform`]), and when the transform would put
/// it in a chain of the other dimension — the nearest transform above it,
/// or below it on any branch, being the other's
/// ([`HierarchyError::MixedDimensions`]). An entity that has it already
/// may: that is an edit of the value, not a new transform.
pub fn can_take_transform(
    world: &World,
    entity: Entity,
    dimension: Dimension,
) -> Result<(), HierarchyError> {
    if dimension.is_on(world, entity) {
        return Ok(());
    }
    if dimension.other().is_on(world, entity) {
        return Err(HierarchyError::SecondTransform);
    }
    let above = parent_of(world, entity).and_then(|parent| nearest_above(world, parent));
    let below: Vec<Dimension> = children_of(world, entity)
        .into_iter()
        .flat_map(|child| nearest_below(world, child))
        .collect();
    let other = dimension.other();
    if above.is_some_and(|(_, dims)| dims.contains(&other)) || below.contains(&other) {
        return Err(HierarchyError::MixedDimensions);
    }
    Ok(())
}

/// The first of `entity` and its ancestors that carries a transform, and
/// the dimensions it carries.
fn nearest_above(world: &World, entity: Entity) -> Option<(Entity, Vec<Dimension>)> {
    std::iter::once(entity)
        .chain(ancestors(world, entity))
        .find_map(|at| {
            let dims = Dimension::of(world, at);
            (!dims.is_empty()).then_some((at, dims))
        })
}

/// The dimensions of the first transform on every branch at or below
/// `entity`. Cycle-safe.
fn nearest_below(world: &World, entity: Entity) -> Vec<Dimension> {
    let mut found = Vec::new();
    let mut seen = HashSet::new();
    let mut frontier = vec![entity];
    while let Some(at) = frontier.pop() {
        if !seen.insert(at) {
            continue;
        }
        let dims = Dimension::of(world, at);
        if dims.is_empty() {
            frontier.extend(children_of(world, at));
        } else {
            for d in dims {
                if !found.contains(&d) {
                    found.push(d);
                }
            }
        }
    }
    found
}

#[cfg(test)]
mod tests;
