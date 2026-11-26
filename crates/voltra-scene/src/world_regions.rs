//! A big world as a file: its regions, each a scene and the rectangle it
//! covers, streamed in and out around the world's anchors.
//!
//! Unreal's World Partition cuts one huge level into grid cells loaded
//! around *streaming sources*; Unity has no built-in answer and games
//! stream additive scenes by hand; Godot leaves it to the game. The shape
//! here is Unreal's, kept to what a 2D world needs and authored as files
//! the engine already has: each region is an ordinary scene, saved and
//! opened in the editor like any other, its entities placed at their world
//! positions, and a `.world.ron` lists them with their bounds. Nothing in a
//! region knows it is one.
//!
//! ```ron
//! (
//!     base: Some(Path("scenes/overworld.ron")),
//!     load_radius: 40.0,
//!     unload_margin: 10.0,
//!     regions: [
//!         (scene: Path("scenes/overworld/0_0.ron"), min: (0.0, 0.0), max: (32.0, 32.0)),
//!         (scene: Path("scenes/overworld/1_0.ron"), min: (32.0, 0.0), max: (64.0, 32.0)),
//!     ],
//! )
//! ```

use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};
use voltra_assets::AssetPath;
use voltra_render::glam::Vec2;

/// What a world file's name ends in.
pub const WORLD_SUFFIX: &str = ".world.ron";

/// The radius an anchor keeps loaded when it names none, in world units.
pub const DEFAULT_LOAD_RADIUS: f32 = 32.0;

/// How much further than its load radius an anchor must go before a region
/// it kept is let go, in world units.
pub const DEFAULT_UNLOAD_MARGIN: f32 = 8.0;

/// A world split into regions, as its `.world.ron` describes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorldRegions {
    /// The scene the regions are streamed around — the one holding the
    /// player, the camera, the HUD. The streamer does not load it (the
    /// game is already in it); it is what the editor draws the region
    /// bounds over.
    pub base: Option<AssetPath>,
    /// How far around an anchor regions are loaded, for an anchor that
    /// names no radius of its own.
    pub load_radius: f32,
    /// How much further a region may be before it is unloaded: the gap
    /// that keeps an anchor walking along a region's edge from loading and
    /// unloading it every few frames.
    pub unload_margin: f32,
    /// The regions, each loaded and unloaded on its own.
    pub regions: Vec<Region>,
}

impl Default for WorldRegions {
    fn default() -> Self {
        Self {
            base: None,
            load_radius: DEFAULT_LOAD_RADIUS,
            unload_margin: DEFAULT_UNLOAD_MARGIN,
            regions: Vec::new(),
        }
    }
}

/// One region: a scene, and the rectangle of the world its entities stand
/// in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Region {
    /// The scene file holding the region's entities, at world positions.
    pub scene: AssetPath,
    /// The corner of the rectangle nearest negative infinity.
    pub min: Vec2,
    /// The opposite corner.
    pub max: Vec2,
}

impl Region {
    /// How far `point` is from the rectangle: zero inside or on it.
    pub fn distance_to(&self, point: Vec2) -> f32 {
        let nearest = point.clamp(self.min, self.max);
        nearest.distance(point)
    }

    /// The rectangle's centre.
    pub fn center(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }
}

/// Why a world file would not read.
#[derive(Debug)]
pub enum WorldRegionsError {
    /// The file could not be read.
    Io(std::io::Error),
    /// The text is not a world description.
    Parse(ron::error::SpannedError),
    /// It parsed, but describes no world that can be streamed.
    Invalid(String),
}

impl fmt::Display for WorldRegionsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "world file i/o failed: {e}"),
            Self::Parse(e) => write!(f, "world file is not valid RON: {e}"),
            Self::Invalid(why) => write!(f, "world file is not usable: {why}"),
        }
    }
}

impl std::error::Error for WorldRegionsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Parse(e) => Some(e),
            Self::Invalid(_) => None,
        }
    }
}

impl WorldRegions {
    /// Reads the world file at `path` under `root`, through the asset reads
    /// (the pack, in a browser), and checks it.
    pub fn read(root: &Path, path: &AssetPath) -> Result<Self, WorldRegionsError> {
        let text = voltra_assets::fs::read_to_string(&root.join(path.as_str()))
            .map_err(WorldRegionsError::Io)?;
        Self::from_ron(&text)
    }

    /// Parses and checks a world description.
    pub fn from_ron(text: &str) -> Result<Self, WorldRegionsError> {
        let world: Self = ron::from_str(text).map_err(WorldRegionsError::Parse)?;
        world.check()?;
        Ok(world)
    }

    /// The description as the text of a world file.
    pub fn to_ron(&self) -> Result<String, ron::Error> {
        ron::ser::to_string_pretty(self, voltra_assets::pretty_ron())
    }

    /// A grid of `columns` × `rows` regions of `cell` each, the first
    /// with its `min` at `origin`; `scene` names the scene of the region at
    /// each column and row. What a generated world — or a test — is made of.
    pub fn grid(
        origin: Vec2,
        cell: Vec2,
        columns: u32,
        rows: u32,
        mut scene: impl FnMut(u32, u32) -> AssetPath,
    ) -> Self {
        let mut regions = Vec::with_capacity(columns as usize * rows as usize);
        for row in 0..rows {
            for column in 0..columns {
                let min = origin + cell * Vec2::new(column as f32, row as f32);
                regions.push(Region {
                    scene: scene(column, row),
                    min,
                    max: min + cell,
                });
            }
        }
        Self {
            regions,
            ..Self::default()
        }
    }

    /// Whether `scene` is this world's base or one of its regions: whether
    /// an editor with `scene` open should draw these bounds.
    pub fn mentions(&self, scene: &AssetPath) -> bool {
        self.base.as_ref() == Some(scene) || self.regions.iter().any(|r| r.scene == *scene)
    }

    /// Refuses what cannot be streamed: a radius or margin that is
    /// negative or not a number, and a rectangle turned inside out.
    pub fn check(&self) -> Result<(), WorldRegionsError> {
        if !(self.load_radius.is_finite() && self.load_radius >= 0.0) {
            return Err(WorldRegionsError::Invalid(format!(
                "load_radius {} is not a distance",
                self.load_radius
            )));
        }
        if !(self.unload_margin.is_finite() && self.unload_margin >= 0.0) {
            return Err(WorldRegionsError::Invalid(format!(
                "unload_margin {} is not a distance",
                self.unload_margin
            )));
        }
        for region in &self.regions {
            let finite = region.min.is_finite() && region.max.is_finite();
            if !finite || region.min.x > region.max.x || region.min.y > region.max.y {
                return Err(WorldRegionsError::Invalid(format!(
                    "region {} has bounds {} to {}",
                    region.scene.as_str(),
                    region.min,
                    region.max
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(raw: &str) -> AssetPath {
        AssetPath::new(raw).expect("valid")
    }

    #[test]
    fn a_world_file_round_trips() {
        let mut world = WorldRegions::grid(Vec2::ZERO, Vec2::splat(10.0), 2, 2, |c, r| {
            path(&format!("scenes/r{c}_{r}.ron"))
        });
        world.base = Some(path("scenes/base.ron"));
        let text = world.to_ron().expect("serialises");
        assert_eq!(WorldRegions::from_ron(&text).expect("reads"), world);
    }

    #[test]
    fn the_documented_example_reads() {
        let text = r#"(
            base: Some(Path("scenes/overworld.ron")),
            load_radius: 40.0,
            regions: [
                (scene: Path("scenes/overworld/0_0.ron"), min: (0.0, 0.0), max: (32.0, 32.0)),
            ],
        )"#;
        let world = WorldRegions::from_ron(text).expect("reads");
        assert_eq!(world.unload_margin, DEFAULT_UNLOAD_MARGIN);
        assert!(world.mentions(&path("scenes/overworld.ron")));
        assert!(world.mentions(&path("scenes/overworld/0_0.ron")));
        assert!(!world.mentions(&path("scenes/elsewhere.ron")));
    }

    #[test]
    fn a_grid_tiles_without_gaps() {
        let world = WorldRegions::grid(Vec2::new(-5.0, 0.0), Vec2::new(10.0, 4.0), 3, 2, |c, r| {
            path(&format!("{c}_{r}.ron"))
        });
        assert_eq!(world.regions.len(), 6);
        assert_eq!(world.regions[0].min, Vec2::new(-5.0, 0.0));
        assert_eq!(world.regions[5].max, Vec2::new(25.0, 8.0));
        assert_eq!(world.regions[1].min, world.regions[0].min + Vec2::X * 10.0);
    }

    #[test]
    fn distance_is_zero_inside_and_to_the_nearest_edge_outside() {
        let region = Region {
            scene: path("r.ron"),
            min: Vec2::ZERO,
            max: Vec2::splat(10.0),
        };
        assert_eq!(region.distance_to(Vec2::splat(5.0)), 0.0);
        assert_eq!(region.distance_to(Vec2::new(13.0, 5.0)), 3.0);
        assert_eq!(region.distance_to(Vec2::new(13.0, 14.0)), 5.0);
    }

    #[test]
    fn inside_out_bounds_and_negative_radii_are_refused() {
        let mut world = WorldRegions::grid(Vec2::ZERO, Vec2::ONE, 1, 1, |_, _| path("a.ron"));
        world.regions[0].max = Vec2::splat(-1.0);
        assert!(matches!(world.check(), Err(WorldRegionsError::Invalid(_))));
        let mut world = WorldRegions {
            load_radius: -1.0,
            ..WorldRegions::default()
        };
        assert!(world.check().is_err());
        world.load_radius = f32::NAN;
        assert!(world.check().is_err());
    }
}
