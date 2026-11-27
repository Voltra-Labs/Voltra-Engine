//! Poses a query reads in place of the world's: how a rewound query
//! ([`crate::rewind`]) asks the world as it was without changing it.

use voltra_ecs::hash::FastMap;
use voltra_ecs::Entity;
use voltra_scene::{Collider, Transform};

/// By entity, the collider and transform a query uses instead of the
/// world's — or `None`, to find nothing of the entity. An entity it does
/// not name is read from the world as it is.
pub(crate) type Poses = FastMap<Entity, Option<(Collider, Transform)>>;
