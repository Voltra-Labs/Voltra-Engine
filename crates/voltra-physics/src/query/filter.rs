//! Which colliders a query may find.

use voltra_ecs::{Entity, World};
use voltra_scene::{CollisionLayers, Sensor};

/// Which colliders a query is allowed to find.
///
/// One mask, not the pair a collider carries: a query is not a collider, it
/// has no layers of its own to be looked at, and only the caller's side of the
/// test exists. Unity's `LayerMask` argument and Rapier's `QueryFilter` are
/// the same one-sided shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueryFilter {
    /// Layers this query looks at. A collider is a candidate when it is on any
    /// of them.
    pub mask: u32,
    /// An entity the query ignores — the caster, almost always. A ground check
    /// that hits the character it started inside answers its own question.
    pub exclude: Option<Entity>,
    /// Whether sensors are found. Off by default: a shot should not stop at a
    /// trigger volume, and a ground check should not stand on one.
    pub sensors: bool,
}

impl Default for QueryFilter {
    fn default() -> Self {
        Self {
            mask: CollisionLayers::ALL,
            exclude: None,
            sensors: false,
        }
    }
}

impl QueryFilter {
    /// Looks at everything solid.
    pub fn new() -> Self {
        Self::default()
    }

    /// Looks only at the layers in `mask`.
    pub const fn looking_at(mut self, mask: u32) -> Self {
        self.mask = mask;
        self
    }

    /// Ignores `entity`.
    pub const fn excluding(mut self, entity: Entity) -> Self {
        self.exclude = Some(entity);
        self
    }

    /// Finds sensors as well as solid colliders.
    pub const fn with_sensors(mut self) -> Self {
        self.sensors = true;
        self
    }

    /// Whether this query may find `entity`.
    pub(super) fn allows(&self, world: &World, entity: Entity) -> bool {
        if self.exclude == Some(entity) {
            return false;
        }
        if !self.sensors && world.get::<Sensor>(entity).is_some() {
            return false;
        }
        let layers = world
            .get::<CollisionLayers>(entity)
            .copied()
            .unwrap_or_default();
        self.mask & layers.layers != 0
    }
}
