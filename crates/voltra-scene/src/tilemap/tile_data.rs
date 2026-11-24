//! What a tile of a tileset is besides its picture: the shape it collides
//! with, and values a game reads by cell — `ladder: true`, `damage: 5`.
//!
//! Godot's TileSet physics layers and custom data layers, Unity's tile
//! collider types and Tiled's tile properties, in one record per frame. Kept
//! on the layer, keyed by frame, beside its [`animations`] and
//! [`terrains`](super::terrain) — the same place everything else a frame
//! means to a layer already lives.
//!
//! [`animations`]: super::Tilemap::animations

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use voltra_render::glam::{IVec2, Vec2};

use super::{Tile, Tilemap};

/// A frame's collision shape and custom values.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TileData {
    /// What part of its cell a tile of this frame collides with, on a layer
    /// with collision. Full — every tile solid, as before — unless set.
    #[serde(skip_serializing_if = "TileShape::is_full")]
    pub shape: TileShape,
    /// Named values a game reads by cell through
    /// [`Tilemap::custom_at`](super::Tilemap::custom_at).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub custom: BTreeMap<String, TileValue>,
}

/// One custom value: what a RON literal is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TileValue {
    /// `true` or `false`.
    Bool(bool),
    /// A whole number, `5`.
    Int(i64),
    /// A number with a point, `0.5`.
    Float(f64),
    /// Text, `"lava"`.
    Text(String),
}

impl TileValue {
    /// The value as a flag: a `Bool` as it is, a number as whether it is not
    /// zero, text as whether there is any.
    pub fn truthy(&self) -> bool {
        match self {
            Self::Bool(value) => *value,
            Self::Int(value) => *value != 0,
            Self::Float(value) => *value != 0.0,
            Self::Text(value) => !value.is_empty(),
        }
    }

    /// The value as a number, if it is one.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Int(value) => Some(*value as f64),
            Self::Float(value) => Some(*value),
            _ => None,
        }
    }

    /// The value as text, if it is text.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value),
            _ => None,
        }
    }
}

/// Which half of a cell a half tile fills.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Side {
    /// The lower half: a low step.
    #[default]
    Bottom,
    /// The upper half: a low ceiling.
    Top,
    /// The left half.
    Left,
    /// The right half.
    Right,
}

/// What part of its cell a tile collides with, as drawn upright — its
/// tile's turns and mirrors turn it too, so one slope frame mirrored is the
/// slope going the other way.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum TileShape {
    /// The whole cell. Neighbouring full tiles merge into one box, as every
    /// solid tile did before shapes existed.
    #[default]
    Full,
    /// Nothing: a tile you walk through on a solid layer — a ladder, a
    /// background tile, a doorway.
    Empty,
    /// Half the cell.
    Half(Side),
    /// A floor rising from `left` to `right`, each a height in `0..=1` of the
    /// cell: `(0, 1)` is a 45° slope, `(0, 0.5)` and `(0.5, 1)` the two tiles
    /// of a gentle one.
    Slope {
        /// The floor's height at the cell's left edge.
        left: f32,
        /// The floor's height at its right edge.
        right: f32,
    },
    /// Any outline in `0..=1` of the cell, `x` right and `y` up from its
    /// lower-left corner. It collides as its convex hull, which may have at
    /// most eight corners: a concave outline fills in, and one needing more
    /// corners is skipped with a warning.
    Polygon(Vec<Vec2>),
}

impl TileShape {
    /// Whether it is the whole cell, which merges with its neighbours.
    pub fn is_full(&self) -> bool {
        matches!(self, Self::Full)
    }

    /// The outline in `0..=1` of the cell, `y` up, as drawn upright, or
    /// `None` for no collision. `Full` is the unit square.
    pub fn outline(&self) -> Option<Vec<Vec2>> {
        let square = |x0: f32, y0: f32, x1: f32, y1: f32| {
            vec![
                Vec2::new(x0, y0),
                Vec2::new(x1, y0),
                Vec2::new(x1, y1),
                Vec2::new(x0, y1),
            ]
        };
        match self {
            Self::Full => Some(square(0.0, 0.0, 1.0, 1.0)),
            Self::Empty => None,
            Self::Half(Side::Bottom) => Some(square(0.0, 0.0, 1.0, 0.5)),
            Self::Half(Side::Top) => Some(square(0.0, 0.5, 1.0, 1.0)),
            Self::Half(Side::Left) => Some(square(0.0, 0.0, 0.5, 1.0)),
            Self::Half(Side::Right) => Some(square(0.5, 0.0, 1.0, 1.0)),
            Self::Slope { left, right } => {
                let left = finite01(*left);
                let right = finite01(*right);
                let mut points = vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0)];
                // A corner of zero height is left out: the outline is a
                // triangle then, not a quad with two points in one place.
                if right > 0.0 {
                    points.push(Vec2::new(1.0, right));
                }
                if left > 0.0 {
                    points.push(Vec2::new(0.0, left));
                }
                (points.len() >= 3).then_some(points)
            }
            Self::Polygon(points) => (points.len() >= 3).then(|| {
                points
                    .iter()
                    .map(|p| Vec2::new(finite01(p.x), finite01(p.y)))
                    .collect()
            }),
        }
    }

    /// [`outline`](Self::outline) turned and mirrored as `tile` is drawn.
    pub fn outline_for(&self, tile: Tile) -> Option<Vec<Vec2>> {
        let outline = self.outline()?;
        Some(
            outline
                .into_iter()
                .map(|p| {
                    // The outline is in the frame's space y up; the tile's
                    // orientation works in image space, y down.
                    let drawn = tile.place(Vec2::new(p.x, 1.0 - p.y));
                    Vec2::new(drawn.x, 1.0 - drawn.y)
                })
                .collect(),
        )
    }
}

impl Tilemap {
    /// The collision shape of a tile of `frame`: its [`TileData`]'s, or the
    /// whole cell.
    pub fn shape_of(&self, frame: u32) -> &TileShape {
        const FULL: TileShape = TileShape::Full;
        self.tile_data.get(&frame).map_or(&FULL, |data| &data.shape)
    }

    /// The custom value `key` of the tile at `cell`, if a tile is there and
    /// its frame has one: how a game asks "is this a ladder?" of a level.
    pub fn custom_at(&self, cell: IVec2, key: &str) -> Option<&TileValue> {
        let tile = self.get(cell)?;
        self.tile_data.get(&tile.frame)?.custom.get(key)
    }
}

fn finite01(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mirrored_slope_rises_the_other_way() {
        let slope = TileShape::Slope {
            left: 0.0,
            right: 1.0,
        };
        let upright = slope.outline_for(Tile::new(0)).expect("a shape");
        assert!(upright.contains(&Vec2::new(1.0, 1.0)));

        let mirrored = slope
            .outline_for(Tile::new(0).mirrored_x())
            .expect("a shape");
        assert!(mirrored.contains(&Vec2::new(0.0, 1.0)), "{mirrored:?}");
        assert!(!mirrored.contains(&Vec2::new(1.0, 1.0)));
    }

    #[test]
    fn a_turned_half_tile_is_the_other_half() {
        let half = TileShape::Half(Side::Bottom);
        // A quarter turn clockwise puts the bottom on the left.
        let turned = half
            .outline_for(Tile::new(0).rotated_cw())
            .expect("a shape");
        assert!(turned.iter().all(|p| p.x <= 0.5), "{turned:?}");
    }

    #[test]
    fn an_empty_tile_and_a_flat_slope_have_no_outline() {
        assert_eq!(TileShape::Empty.outline(), None);
        let flat = TileShape::Slope {
            left: 0.0,
            right: 0.0,
        };
        assert_eq!(flat.outline(), None);
    }

    #[test]
    fn custom_values_read_as_ron_literals() {
        let text = r#"(shape: Slope(left: 0.0, right: 0.5), custom: {"ladder": true, "damage": 5, "speed": 0.5, "kind": "lava"})"#;
        let data: TileData = ron::from_str(text).expect("reads");

        assert_eq!(data.custom["ladder"], TileValue::Bool(true));
        assert_eq!(data.custom["damage"], TileValue::Int(5));
        assert_eq!(data.custom["speed"], TileValue::Float(0.5));
        assert_eq!(data.custom["kind"].as_str(), Some("lava"));
        let back: TileData = ron::from_str(&ron::to_string(&data).expect("writes")).expect("reads");
        assert_eq!(back, data);
        assert_eq!(
            ron::to_string(&TileData::default()).expect("writes"),
            "()",
            "a plain tile writes nothing"
        );
    }
}
