//! A streamed world's region bounds, drawn over the scene being edited.
//!
//! The editor's whole affordance for big worlds, kept small on purpose: a
//! region is an ordinary scene, edited like any other, and its bounds are
//! one line each in a `.world.ron` the author writes by hand or a tool
//! generates. What the author cannot see without help is where those
//! rectangles fall, so with **Scene ▸ Show region bounds** on, every
//! world file under the asset root that names the open scene — as its
//! base or as one of its regions — is drawn: each region's rectangle, the
//! open scene's own brighter. Unreal's World Partition editor goes much
//! further (a minimap, loading regions into the editor, cell sizes); a
//! world editor of that kind is a stage of its own.

use std::path::{Path, PathBuf};

use voltra_assets::AssetPath;
use voltra_render::glam::Vec2;
use voltra_render::LineBatch;
use voltra_scene::world_regions::WORLD_SUFFIX;
use voltra_scene::WorldRegions;

/// A region's outline. Cyan: unused by the gizmos, the physics overlay
/// (green, magenta) and the camera frame (amber).
const COLOR: [f32; 4] = [0.3, 0.85, 0.95, 0.45];

/// The open scene's own region, if it is one.
const OWN_COLOR: [f32; 4] = [0.3, 0.85, 0.95, 0.95];

/// Outline thickness, in pixels. Matches the camera overlay's.
const WIDTH: f32 = 1.5;

/// How deep under the root world files are looked for.
const MAX_DEPTH: usize = 8;

/// Whether the bounds are shown, and the worlds read for the scene open.
#[derive(Debug, Default)]
pub struct RegionOverlay {
    /// Whether the bounds are drawn.
    pub show: bool,
    /// The root and scene the worlds below were found for, so they are read
    /// again only when either changes or the toggle is turned on again.
    read_for: Option<(PathBuf, Option<AssetPath>)>,
    worlds: Vec<WorldRegions>,
}

impl RegionOverlay {
    /// Reads the world files again on the next draw: what turning the
    /// toggle on does, so an edited file is seen.
    pub fn rescan(&mut self) {
        self.read_for = None;
    }

    /// Finds the worlds naming `scene` under `root`, unless they were found
    /// for these two already.
    pub fn refresh(&mut self, root: &Path, scene: Option<&AssetPath>) {
        let key = (root.to_path_buf(), scene.cloned());
        if self.read_for.as_ref() == Some(&key) {
            return;
        }
        self.worlds = match scene {
            Some(scene) => worlds_naming(root, scene),
            None => Vec::new(),
        };
        self.read_for = Some(key);
    }

    /// Pushes every region's outline into `lines`; the region that is the
    /// open scene `scene` brighter.
    pub fn draw(&self, scene: Option<&AssetPath>, lines: &mut LineBatch) {
        for world in &self.worlds {
            for region in &world.regions {
                let color = if Some(&region.scene) == scene {
                    OWN_COLOR
                } else {
                    COLOR
                };
                rect(region.min, region.max, color, lines);
            }
        }
    }
}

/// Every readable world file under `root` that names `scene`. One that
/// will not read is logged and skipped: the overlay is a view, and a
/// broken file elsewhere must not hide the others.
fn worlds_naming(root: &Path, scene: &AssetPath) -> Vec<WorldRegions> {
    let mut files = Vec::new();
    collect(root, 0, &mut files);
    files.sort();
    files
        .into_iter()
        .filter_map(|file| {
            let text = std::fs::read_to_string(&file)
                .map_err(|e| log::warn!("{}: {e}", file.display()))
                .ok()?;
            match WorldRegions::from_ron(&text) {
                Ok(world) => world.mentions(scene).then_some(world),
                Err(e) => {
                    log::warn!("{}: {e}", file.display());
                    None
                }
            }
        })
        .collect()
}

fn collect(dir: &Path, depth: usize, files: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect(&path, depth + 1, files);
        } else if name.ends_with(WORLD_SUFFIX) {
            files.push(path);
        }
    }
}

/// The four edges of `min..max`.
fn rect(min: Vec2, max: Vec2, color: [f32; 4], lines: &mut LineBatch) {
    let corners = [min, Vec2::new(max.x, min.y), max, Vec2::new(min.x, max.y)];
    for i in 0..corners.len() {
        lines.push(corners[i], corners[(i + 1) % corners.len()], WIDTH, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh directory per test, as the editor's other file tests make.
    fn scratch_root() -> voltra_testkit::Scratch {
        voltra_testkit::Scratch::new("voltra-regions")
    }

    fn path(raw: &str) -> AssetPath {
        AssetPath::new(raw).expect("valid")
    }

    fn write(root: &Path, name: &str, world: &WorldRegions) {
        let full = root.join(name);
        std::fs::create_dir_all(full.parent().expect("a parent")).expect("dir");
        std::fs::write(full, world.to_ron().expect("serialises")).expect("writes");
    }

    #[test]
    fn the_worlds_naming_the_open_scene_draw_four_edges_a_region() {
        let root = scratch_root();
        let mut world = WorldRegions::grid(Vec2::ZERO, Vec2::splat(16.0), 3, 2, |c, r| {
            path(&format!("scenes/r{c}_{r}.ron"))
        });
        world.base = Some(path("scenes/overworld.ron"));
        write(&root, "worlds/over.world.ron", &world);
        // Another world, naming nothing open: not drawn.
        let other = WorldRegions::grid(Vec2::ZERO, Vec2::ONE, 1, 1, |_, _| path("x.ron"));
        write(&root, "other.world.ron", &other);
        std::fs::write(root.join("broken.world.ron"), "not ron").expect("writes");

        let mut overlay = RegionOverlay::default();
        let scene = path("scenes/overworld.ron");
        overlay.refresh(&root, Some(&scene));
        let mut lines = LineBatch::default();
        overlay.draw(Some(&scene), &mut lines);
        assert_eq!(lines.len(), 6 * 4);

        // Opening one of its regions draws the same world.
        let region = path("scenes/r1_0.ron");
        overlay.refresh(&root, Some(&region));
        let mut lines = LineBatch::default();
        overlay.draw(Some(&region), &mut lines);
        assert_eq!(lines.len(), 6 * 4);
    }

    #[test]
    fn an_unsaved_scene_draws_nothing() {
        let root = scratch_root();
        let world = WorldRegions::grid(Vec2::ZERO, Vec2::ONE, 2, 2, |c, r| {
            path(&format!("{c}_{r}.ron"))
        });
        write(&root, "a.world.ron", &world);
        let mut overlay = RegionOverlay::default();
        overlay.refresh(&root, None);
        let mut lines = LineBatch::default();
        overlay.draw(None, &mut lines);
        assert!(lines.is_empty());
    }
}
