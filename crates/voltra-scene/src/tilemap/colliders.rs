//! A solid layer's tiles, as ordinary colliders the physics already handles.
//!
//! **Generated entities, not a new shape.** The simulation knows one collider
//! per entity and keys every contact, cached impulse and event by entity. A
//! compound "tilemap shape" would have to teach all of that to address a
//! *part* of an entity; a static box per merged run of tiles needs nothing
//! taught. Unity's `TilemapCollider2D` + `CompositeCollider2D` and Godot's
//! tile physics both end up as plain static shapes in the physics server for
//! the same reason.
//!
//! **Not part of the scene.** A generated collider has no [`SceneId`], so it
//! is never saved, never listed in the hierarchy and never picked; it is
//! derived data, rebuilt from the layer, the way a mesh is. The layer is the
//! thing authored and undone, and its colliders follow.
//!
//! **Full tiles merge; other shapes are a polygon each** (stage 98). A tile
//! whose frame has a [`TileShape`](super::TileShape) other than full — a half
//! tile, a slope, a custom outline — is one static convex polygon of its own,
//! turned as its tile is drawn; full tiles merge into boxes as before; an
//! empty shape adds nothing. Merging slopes would need a concave outline the
//! physics does not have, and a slope is rarely more than a few tiles.
//!
//! **Rebuilt when stale, checked every frame.** What a layer's colliders were
//! built from is kept on them; [`sync_colliders`] compares it with the layer
//! as it is now and rebuilds the layer whose inputs moved — a cell, the tile
//! size, the layer's place in the world, its collision filter. Comparing is a
//! handful of values per layer, so the check is free until something changes.
//!
//! [`SceneId`]: crate::SceneId

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use voltra_ecs::{Entity, World};
use voltra_render::glam::{Mat3, Vec2};

use super::{solid_boxes, Tilemap};
use crate::collider::ConvexPolygon;
use crate::hierarchy::world_matrix;
use crate::{Collider, CollisionLayers, PhysicsMaterial, RigidBody, Sensor, Transform};

/// Marks a collider generated from a solid tilemap layer.
///
/// Collision events name this entity, not the layer; `layer` is the way back,
/// for a game that wants to know it landed on "the ground".
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileCollider {
    /// The layer whose tiles this box covers.
    pub layer: Entity,
    built_from: Inputs,
}

/// Everything about a layer a generated box depends on.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Inputs {
    revision: u64,
    /// The frames' shapes, hashed: a shape changed in the inspector moves
    /// no cell, so the revision alone would miss it.
    shapes: u64,
    tile_size: Vec2,
    world: Mat3,
    layers: Option<CollisionLayers>,
    material: Option<PhysicsMaterial>,
    sensor: bool,
}

impl Inputs {
    fn of(world: &World, layer: Entity, map: &Tilemap) -> Self {
        Self {
            revision: map.revision().get(),
            shapes: shapes_hash(map),
            tile_size: map.tile_size,
            world: world_matrix(world, layer),
            layers: world.get::<CollisionLayers>(layer).copied(),
            material: world.get::<PhysicsMaterial>(layer).copied(),
            sensor: world.get::<Sensor>(layer).is_some(),
        }
    }
}

/// Brings every solid layer's generated colliders up to date, and despawns
/// those whose layer is gone or no longer solid.
///
/// Call once a frame before the physics steps, and after anything inside the
/// steps that may have edited a layer.
pub fn sync_colliders(world: &mut World) {
    let solid: BTreeMap<Entity, Inputs> = world
        .query::<Tilemap>()
        .filter(|(_, map)| map.collision)
        .map(|(layer, map)| (layer, Inputs::of(world, layer, map)))
        .collect();

    let mut generated: BTreeMap<Entity, Vec<(Entity, Inputs)>> = BTreeMap::new();
    for (entity, collider) in world.query::<TileCollider>() {
        generated
            .entry(collider.layer)
            .or_default()
            .push((entity, collider.built_from));
    }

    let mut stale = Vec::new();
    let mut current = Vec::new();
    for (layer, boxes) in generated {
        let fresh = solid
            .get(&layer)
            .is_some_and(|now| boxes.iter().all(|(_, built)| built == now));
        if fresh {
            current.push(layer);
        } else {
            stale.extend(boxes.into_iter().map(|(entity, _)| entity));
        }
    }
    for entity in stale {
        world.despawn(entity);
    }

    // A solid layer with no colliders is rebuilt every frame — which costs
    // nothing for the one kind that has none, an empty layer.
    for (layer, inputs) in solid {
        if !current.contains(&layer) {
            build(world, layer, inputs);
        }
    }
}

/// A hash of every frame's shape — what the colliders are cut by besides
/// the cells.
fn shapes_hash(map: &Tilemap) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for (frame, data) in &map.tile_data {
        if data.shape.is_full() {
            continue;
        }
        frame.hash(&mut hasher);
        // Through its text: `TileShape` holds floats, which do not hash, and
        // the shapes of a layer are a handful of short strings.
        format!("{:?}", data.shape).hash(&mut hasher);
    }
    hasher.finish()
}

/// Spawns `layer`'s colliders, built from `inputs`.
fn build(world: &mut World, layer: Entity, inputs: Inputs) {
    let Some(map) = world.get::<Tilemap>(layer) else {
        return;
    };
    // A zero-sized tile has no area to collide with; the inspector clamps it,
    // but a scene file or a game can still write one.
    if inputs.tile_size.x == 0.0 || inputs.tile_size.y == 0.0 {
        return;
    }
    let mut shapes: Vec<(Vec2, Collider)> = solid_boxes(map)
        .into_iter()
        .map(|cells| {
            let min = cells.min.as_vec2() * inputs.tile_size;
            let max = cells.max.as_vec2() * inputs.tile_size;
            let half_extents = ((max - min) * 0.5).abs();
            ((min + max) * 0.5, Collider::Box { half_extents })
        })
        .collect();
    shapes.extend(polygons(map, inputs.tile_size));

    for (centre, collider) in shapes {
        // The layer's own frame, with the shape's centre carried through it:
        // the shape turns and scales with the layer, and its extents stay in
        // the layer's units for the transform's scale to apply, as any
        // collider's do.
        let mut transform = Transform::from_matrix(inputs.world);
        transform.translation = inputs.world.transform_point2(centre);

        let entity = world.spawn();
        world.insert(entity, transform);
        world.insert(entity, RigidBody::new_static());
        world.insert(entity, collider);
        if let Some(layers) = inputs.layers {
            world.insert(entity, layers);
        }
        if let Some(material) = inputs.material {
            world.insert(entity, material);
        }
        if inputs.sensor {
            world.insert(entity, Sensor);
        }
        world.insert(
            entity,
            TileCollider {
                layer,
                built_from: inputs,
            },
        );
    }
}

/// One polygon per tile whose shape is neither full nor empty, centred on
/// its cell, in the layer's units. An outline that is not a convex polygon
/// of at most eight points is skipped with a warning naming its frame.
fn polygons(map: &Tilemap, tile_size: Vec2) -> Vec<(Vec2, Collider)> {
    let mut out = Vec::new();
    let mut warned = Vec::new();
    for (cell, tile) in map.tiles() {
        let shape = map.shape_of(tile.frame);
        if shape.is_full() {
            continue;
        }
        let Some(outline) = shape.outline_for(tile) else {
            continue;
        };
        let centre = (cell.as_vec2() + Vec2::splat(0.5)) * tile_size;
        let points: Vec<Vec2> = outline
            .iter()
            .map(|p| (*p - Vec2::splat(0.5)) * tile_size)
            .collect();
        match ConvexPolygon::new(&points) {
            Ok(points) => out.push((
                centre,
                Collider::Polygon {
                    points,
                    radius: 0.0,
                },
            )),
            Err(e) => {
                if !warned.contains(&tile.frame) {
                    warned.push(tile.frame);
                    log::warn!("tile frame {}'s shape is not a collider: {e}", tile.frame);
                }
            }
        }
    }
    out
}
