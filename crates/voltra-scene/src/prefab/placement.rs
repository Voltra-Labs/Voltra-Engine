//! Where an instance is put: its root's transform, whichever dimension it
//! is in.
//!
//! An instance's placement is the one thing about it that is the scene's
//! and never the file's. Every place that handles it — a spawn placing the
//! roots, a sync leaving the root's transform alone, a comparison that never
//! counts it as an override, Apply keeping the file's own, a fragment zeroed
//! at its root — names it through [`Placement`], so a 3D root is handled the
//! way a 2D one always was. Missing one would let a moved 3D instance root
//! record an override, and Apply write a world placement into the prefab.

use ron::value::RawValue;
use voltra_ecs::{Entity, World};
use voltra_render::glam::{Quat, Vec2, Vec3};

use crate::format::{EntityRecord, SceneError};
use crate::hierarchy::Dimension;
use crate::transform::Transform2D;
use crate::transform3d::Transform3D;

/// Where a spawn puts the roots of a prefab: composed in front of a root's
/// own transform of the same dimension.
///
/// `From` either transform, so `spawn(…, &Transform2D::from_translation(at))`
/// reads as it always did and a 3D caller writes the same with a
/// [`Transform3D`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Placement {
    /// Where the file put them: nothing is moved, in either dimension. A
    /// variant's base, an entity a server replicates, a copy made in place.
    #[default]
    AsAuthored,
    /// Around a 2D transform: places roots carrying a [`Transform2D`].
    D2(Transform2D),
    /// Around a 3D transform: places roots carrying a [`Transform3D`].
    D3(Transform3D),
}

impl From<Transform2D> for Placement {
    fn from(at: Transform2D) -> Self {
        Self::D2(at)
    }
}

impl From<&Transform2D> for Placement {
    fn from(at: &Transform2D) -> Self {
        Self::D2(*at)
    }
}

impl From<Transform3D> for Placement {
    fn from(at: Transform3D) -> Self {
        Self::D3(at)
    }
}

impl From<&Transform3D> for Placement {
    fn from(at: &Transform3D) -> Self {
        Self::D3(*at)
    }
}

impl Placement {
    /// The registered names of the components a placement is: a root's
    /// transform of either dimension.
    pub const NAMES: [&'static str; 2] = [Transform2D::NAME, Transform3D::NAME];

    /// Whether the component registered as `name` is an instance root's
    /// placement.
    pub fn names(name: &str) -> bool {
        Self::NAMES.contains(&name)
    }

    /// Whether placing by this moves nothing.
    pub fn is_identity(&self) -> bool {
        match self {
            Self::AsAuthored => true,
            Self::D2(at) => *at == Transform2D::default(),
            Self::D3(at) => *at == Transform3D::default(),
        }
    }

    /// Places `root`: its transform of this placement's dimension composed
    /// behind this one. A root with no transform is not positional and is
    /// not given one. A root of the other dimension cannot be placed by
    /// this — a 2D offset has no meaning in 3D — and is left where the file
    /// put it, warned about unless this moves nothing anyway.
    pub(super) fn place(&self, world: &mut World, root: Entity) {
        if self.is_identity() {
            return;
        }
        let placed = match self {
            Self::AsAuthored => return,
            Self::D2(at) => world
                .get_mut::<Transform2D>(root)
                .map(|local| *local = place_2d(at, local))
                .is_some(),
            Self::D3(at) => world
                .get_mut::<Transform3D>(root)
                .map(|local| *local = place_3d(at, local))
                .is_some(),
        };
        let other = match self {
            Self::D2(_) => world.get::<Transform3D>(root).is_some(),
            Self::D3(_) => world.get::<Transform2D>(root).is_some(),
            Self::AsAuthored => false,
        };
        if !placed && other {
            log::warn!(
                "a prefab root of the other dimension cannot be placed by a {} placement; it \
                 stays where its file put it",
                match self {
                    Self::D3(_) => "3D",
                    _ => "2D",
                }
            );
        }
    }
}

/// The dimensions a stored record has a transform of.
pub(super) fn record_dimensions(record: &EntityRecord) -> Vec<Dimension> {
    [Dimension::D2, Dimension::D3]
        .into_iter()
        .filter(|d| record.components.contains_key(d.transform_name()))
        .collect()
}

/// Each transform `entity` carries, its translation zeroed and its rotation
/// and scale kept, by registered name: a root as a prefab file stores it.
pub(super) fn at_origin(
    world: &World,
    entity: Entity,
) -> Result<Vec<(&'static str, Box<RawValue>)>, SceneError> {
    let mut out = Vec::new();
    if let Some(t) = world.get::<Transform2D>(entity) {
        let zeroed = Transform2D {
            translation: Vec2::ZERO,
            ..*t
        };
        out.push((
            Transform2D::NAME,
            RawValue::from_rust(&zeroed).map_err(SceneError::Serialize)?,
        ));
    }
    if let Some(t) = world.get::<Transform3D>(entity) {
        let zeroed = Transform3D {
            translation: Vec3::ZERO,
            ..*t
        };
        out.push((
            Transform3D::NAME,
            RawValue::from_rust(&zeroed).map_err(SceneError::Serialize)?,
        ));
    }
    Ok(out)
}

/// `local` as seen through `at`.
///
/// A placement that only moves is applied as an addition rather than through
/// a matrix: it is the common case — a bullet at the muzzle, a coin on a tile
/// — and the decomposition [`Transform2D::from_matrix`] does would put float
/// noise into an authored rotation and scale that nothing asked to change.
fn place_2d(at: &Transform2D, local: &Transform2D) -> Transform2D {
    if at.rotation == 0.0 && at.scale == Vec2::ONE {
        return Transform2D {
            translation: local.translation + at.translation,
            ..*local
        };
    }
    Transform2D::from_matrix(at.matrix() * local.matrix())
}

/// [`place_2d`]'s twin. A placement that turns but does not scale composes
/// the quaternions directly rather than decomposing a matrix, for the same
/// reason: the authored scale comes through to the bit.
fn place_3d(at: &Transform3D, local: &Transform3D) -> Transform3D {
    if at.scale == Vec3::ONE {
        if at.rotation == Quat::IDENTITY {
            return Transform3D {
                translation: local.translation + at.translation,
                ..*local
            };
        }
        return Transform3D {
            translation: at.translation + at.rotation * local.translation,
            rotation: (at.rotation * local.rotation).normalize(),
            scale: local.scale,
        };
    }
    Transform3D::from_matrix(at.matrix() * local.matrix())
}
