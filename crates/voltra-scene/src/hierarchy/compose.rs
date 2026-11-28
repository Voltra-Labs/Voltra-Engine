//! Composing a hierarchy, once for both dimensions.
//!
//! A [`Transform2D`] composes as a `Mat3` and a [`Transform3D`] as an
//! `Affine3A`, and that is the whole difference: the walk up a chain, the
//! memoised pass over a world, a grouping node with no transform adding no
//! frame, a cycle composed as a root — all of it is the same work. So it is
//! written once here, generic over [`Local`], and the per-dimension types
//! ([`super::WorldTransforms2D`], [`super::WorldTransforms3D`]) and lookups
//! (`world_matrix_2d`, `world_matrix_3d`, …) are thin names over it.
//!
//! **A chain composes only the transforms of its own dimension.** An
//! ancestor carrying the other one adds no frame, as one carrying none does:
//! a 2D child under a 3D parent has no meaning to compose, and
//! [`super::set_parent`] refuses to make one. A file that has one anyway is
//! warned about on load ([`super::dimension`]) and drawn as if the other
//! dimension's link were not there.

use std::ops::Mul;

use voltra_ecs::{Entity, World};
use voltra_render::glam::{Affine3A, Mat3};

use super::parent::Parent;
use super::walk::ancestors;
use crate::transform::Transform2D;
use crate::transform3d::Transform3D;

/// A transform component that composes down a hierarchy.
pub(crate) trait Local: Copy + 'static {
    /// What it composes as.
    type Matrix: Copy + Mul<Output = Self::Matrix>;
    /// The frame that adds nothing.
    const IDENTITY: Self::Matrix;
    /// Local space to parent space.
    fn local(&self) -> Self::Matrix;
    /// The transform `matrix` describes; lossy for shear.
    fn from_matrix(matrix: Self::Matrix) -> Self;
    /// Whether `matrix` has collapsed and cannot be inverted: inverting a
    /// singular matrix does not panic, it returns infinities and NaN, and a
    /// NaN transform removes an entity from the screen with no error.
    fn collapsed(matrix: &Self::Matrix) -> bool;
    /// The inverse of an invertible `matrix`.
    fn inverse(matrix: &Self::Matrix) -> Self::Matrix;
}

impl Local for Transform2D {
    type Matrix = Mat3;
    const IDENTITY: Mat3 = Mat3::IDENTITY;
    fn local(&self) -> Mat3 {
        self.matrix()
    }
    fn from_matrix(matrix: Mat3) -> Self {
        Transform2D::from_matrix(matrix)
    }
    /// Picking's absolute area threshold, kept: in 2D it bites only below
    /// a millionth of a unit per axis, under a pixel at any zoom.
    fn collapsed(matrix: &Mat3) -> bool {
        matrix.determinant().abs() < MIN_DETERMINANT_2D
    }
    fn inverse(matrix: &Mat3) -> Mat3 {
        matrix.inverse()
    }
}

impl Local for Transform3D {
    type Matrix = Affine3A;
    const IDENTITY: Affine3A = Affine3A::IDENTITY;
    fn local(&self) -> Affine3A {
        self.matrix()
    }
    fn from_matrix(matrix: Affine3A) -> Self {
        Transform3D::from_matrix(matrix)
    }
    /// Relative to the axes' lengths, so a small but whole frame is not
    /// collapsed; see [`Transform3D::is_collapsed`].
    fn collapsed(matrix: &Affine3A) -> bool {
        Transform3D::is_collapsed(matrix)
    }
    fn inverse(matrix: &Affine3A) -> Affine3A {
        matrix.inverse()
    }
}

/// Below this a 2D parent's frame has collapsed and cannot be inverted:
/// picking's threshold, and 0.4's.
const MIN_DETERMINANT_2D: f32 = 1e-12;

/// `entity`'s own `T` as a matrix, or the identity if it has none.
///
/// An entity with children but no transform of its own is a grouping node, and
/// the identity is exactly what "adds no frame of its own" means.
pub(crate) fn local_matrix<T: Local>(world: &World, entity: Entity) -> T::Matrix {
    world.get::<T>(entity).map_or(T::IDENTITY, T::local)
}

/// The frame `entity` sits in: every ancestor's `T`, composed.
pub(crate) fn parent_matrix<T: Local>(world: &World, entity: Entity) -> T::Matrix {
    ancestors(world, entity)
        .iter()
        .rev()
        .fold(T::IDENTITY, |above, ancestor| {
            above * local_matrix::<T>(world, *ancestor)
        })
}

/// Where `entity` is in world space, as a matrix.
pub(crate) fn world_matrix<T: Local>(world: &World, entity: Entity) -> T::Matrix {
    parent_matrix::<T>(world, entity) * local_matrix::<T>(world, entity)
}

/// Puts `entity` at `target` in world space by writing its local `T`;
/// whether it was written. See `set_world_transform_2d` for the cases that
/// leave it alone.
pub(crate) fn set_world<T: Local>(world: &mut World, entity: Entity, target: &T) -> bool {
    let parent = parent_matrix::<T>(world, entity);
    if T::collapsed(&parent) {
        log::warn!("{entity:?} sits under a collapsed parent; leaving its transform alone");
        return false;
    }
    let local = T::from_matrix(T::inverse(&parent) * target.local());
    match world.get_mut::<T>(entity) {
        Some(transform) => {
            *transform = local;
            true
        }
        None => false,
    }
}

/// Every entity's world matrix of one dimension, keyed by handle.
///
/// Indexed by [`Entity::index`]: the entity it was composed for (so a
/// recycled index with a new generation misses) and its matrix. Dense,
/// because the ECS hands out indices densely; a lookup is an index and a
/// compare, not a hash.
#[derive(Debug, Clone)]
pub(crate) struct Composed<M> {
    matrices: Vec<Option<(Entity, M)>>,
    len: usize,
}

impl<M> Default for Composed<M> {
    fn default() -> Self {
        Self {
            matrices: Vec::new(),
            len: 0,
        }
    }
}

impl<M: Copy> Composed<M> {
    /// `entity`'s matrix, or `None` if it was not composed.
    pub(crate) fn get(&self, entity: Entity) -> Option<M> {
        match self.matrices.get(entity.index() as usize) {
            Some(Some((owner, matrix))) if *owner == entity => Some(*matrix),
            _ => None,
        }
    }

    /// `entity`'s matrix to change in place, if it was composed.
    pub(crate) fn get_mut(&mut self, entity: Entity) -> Option<&mut M> {
        match self.matrices.get_mut(entity.index() as usize) {
            Some(Some((owner, matrix))) if *owner == entity => Some(matrix),
            _ => None,
        }
    }

    /// How many entities have a matrix.
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// Every entity composed.
    pub(crate) fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.matrices.iter().flatten().map(|(entity, _)| *entity)
    }

    fn insert(&mut self, entity: Entity, matrix: M) {
        let index = entity.index() as usize;
        if index >= self.matrices.len() {
            self.matrices.resize_with(index + 1, || None);
        }
        if self.matrices[index].is_none() {
            self.len += 1;
        }
        self.matrices[index] = Some((entity, matrix));
    }

    /// Empties this, keeping its storage, and composes every entity with a
    /// `T` in `world` into it.
    ///
    /// Each chain is walked up only until it meets an ancestor already
    /// composed, so every matrix is multiplied once however deep the tree.
    /// Locals and parents are read straight from the world's sparse sets —
    /// an index each — rather than copied into maps first, which was most
    /// of what this cost with thousands of entities.
    ///
    /// A [`Parent`] naming a despawned entity, or one in a cycle, makes its
    /// child a root: drawn where its own transform says, and warned about.
    pub(crate) fn compose<T: Local<Matrix = M>>(&mut self, world: &World)
    where
        M: Mul<Output = M>,
    {
        self.matrices.clear();
        self.len = 0;
        let mut chain: Vec<Entity> = Vec::new();
        // Each set found once, not once per entity asked.
        let parents = world.components::<Parent>();
        let Some(transforms) = world.components::<T>() else {
            return;
        };
        for (entity, transform) in transforms.iter() {
            if self.get(entity).is_some() {
                continue;
            }

            chain.clear();
            let mut cursor = entity;
            let mut above = T::IDENTITY;
            loop {
                chain.push(cursor);
                let Some(parent) = parents
                    .and_then(|parents| parents.get(cursor))
                    .and_then(|link| link.entity)
                    .filter(|parent| world.is_alive(*parent))
                else {
                    break;
                };
                if let Some(known) = self.get(parent) {
                    above = known;
                    break;
                }
                if chain.contains(&parent) {
                    log::warn!("{parent:?} is in a parent cycle; composing it as a root");
                    break;
                }
                cursor = parent;
            }

            for link in chain.iter().rev().copied() {
                // The query's own transform for the entity itself, rather
                // than asking the world for it again.
                above = above
                    * if link == entity {
                        transform.local()
                    } else {
                        // A grouping node without a transform adds no frame.
                        transforms.get(link).map_or(T::IDENTITY, T::local)
                    };
                self.insert(link, above);
            }
        }
    }
}
