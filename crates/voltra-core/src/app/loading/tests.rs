//! Fixtures shared by the loading tests: generated scenes and worlds in a
//! scratch asset root.

mod scene_change;
mod streaming;

use std::path::Path;
use std::time::Duration;

use voltra_assets::AssetPath;
use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_scene::{hierarchy, ComponentRegistry, Name, SceneId, Transform, WorldRegions};

use super::LoadStats;

const FRAME: f32 = 1.0 / 60.0;

/// The rest of a frame — the draw, the present — which a test sleeps
/// through, as a real frame leaves the pool that time to read in.
const REST_OF_FRAME: Duration = Duration::from_millis(1);

/// What a frame may spend past its budget beyond its longest unit: the
/// bookkeeping after the last unit and a timer read, on a shared machine.
const SLACK: Duration = Duration::from_millis(1);

/// One frame of the app, and the rest of it.
fn run_frame(app: &mut crate::app::App) {
    app.advance(FRAME);
    std::thread::sleep(REST_OF_FRAME);
}

fn path(raw: &str) -> AssetPath {
    AssetPath::new(raw).expect("a valid asset path")
}

/// Writes `world` as a scene at `root/name`.
fn write_world(root: &Path, name: &str, world: &World) {
    let file = voltra_scene::format::to_scene_file(world, &ComponentRegistry::with_defaults())
        .expect("saves");
    let full = root.join(name);
    std::fs::create_dir_all(full.parent().expect("a parent")).expect("dir");
    std::fs::write(full, ron::to_string(&file).expect("serialises")).expect("writes");
}

/// A scene of `groups` roots with `children` each, named `<tag>/<n>`, laid
/// out inside `min..min + size`.
fn write_scene(
    root: &Path,
    name: &str,
    tag: &str,
    min: Vec2,
    size: Vec2,
    groups: usize,
    children: usize,
) {
    let mut world = World::new();
    let mut n = 0;
    for group in 0..groups {
        let at = min + size * Vec2::new((group as f32 + 0.5) / groups as f32, 0.5);
        let parent = named(&mut world, &format!("{tag}/{n}"), at);
        n += 1;
        for _ in 0..children {
            let child = named(&mut world, &format!("{tag}/{n}"), Vec2::new(0.0, 0.25));
            n += 1;
            hierarchy::set_parent(&mut world, child, parent).expect("parents");
        }
    }
    write_world(root, name, &world);
}

fn named(world: &mut World, name: &str, at: Vec2) -> voltra_ecs::Entity {
    let entity = world.spawn();
    world.insert(entity, SceneId::new());
    world.insert(entity, Name::new(name));
    world.insert(entity, Transform::from_translation(at));
    entity
}

/// A `columns` × `rows` world of `cell`-sized regions, each a scene of
/// `per_region` entities tagged `r<column>_<row>`, under `root`; returns
/// the world file's path.
fn write_grid(root: &Path, columns: u32, rows: u32, cell: f32, per_region: usize) -> AssetPath {
    let groups = per_region / 5;
    let world = WorldRegions::grid(Vec2::ZERO, Vec2::splat(cell), columns, rows, |c, r| {
        path(&format!("world/r{c}_{r}.ron"))
    });
    for region in &world.regions {
        let tag = region
            .scene
            .as_str()
            .trim_start_matches("world/")
            .trim_end_matches(".ron")
            .to_owned();
        write_scene(
            root,
            region.scene.as_str(),
            &tag,
            region.min,
            region.max - region.min,
            groups,
            4,
        );
    }
    let text = world.to_ron().expect("serialises");
    std::fs::write(root.join("big.world.ron"), text).expect("writes");
    path("big.world.ron")
}

/// Every frame's load work, held to its budget.
#[derive(Debug, Default)]
struct BudgetCheck {
    frames: usize,
    over: Vec<(usize, LoadStats)>,
}

impl BudgetCheck {
    /// The loader's own promise, every frame without exception: no unit
    /// started once the budget was spent (but the frame's first, which
    /// always runs). And the frame's whole load work, in wall time, within
    /// the budget plus the unit it was in when it ran out — noted, not
    /// failed, here: see [`finish`](Self::finish).
    fn frame(&mut self, stats: &LoadStats) {
        assert!(
            stats.units <= 1 || stats.last_start <= stats.budget,
            "frame {}: a unit started {:?} in, past a {:?} budget",
            self.frames,
            stats.last_start,
            stats.budget
        );
        if stats.work > stats.budget + stats.longest_unit + SLACK {
            self.over.push((self.frames, *stats));
        }
        self.frames += 1;
    }

    /// Wall time on a shared machine: a frame preempted between units —
    /// the tests run in parallel, every one with its pool — runs long
    /// through no unit of its own. One frame in twenty may; more is the
    /// loader's doing.
    fn finish(self) {
        assert!(
            self.over.len() * 20 <= self.frames.max(20),
            "{} of {} frames over budget: {:?}",
            self.over.len(),
            self.frames,
            &self.over[..self.over.len().min(5)]
        );
    }
}
