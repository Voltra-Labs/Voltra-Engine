//! One scene file brought into the world over frames: the part a scene
//! change and a streamed region share.
//!
//! In phases, each advanced a unit at a time by [`App::step_build`]:
//!
//! 1. **Reading**, on the pool: the file read, the RON parsed, every
//!    registered component checked against its type, the spawn order
//!    worked out and the textures it names listed — everything that needs
//!    no world. A file that fails here has touched nothing.
//! 2. **Decoding**, on the pool: each texture not in the store yet,
//!    inflated (see `decodes.rs`); uploaded on the frame, one a unit.
//! 3. **Spawning**, on the frame: one entity a unit, complete when it lands
//!    — components read, parent linked, assets resolved.
//! 4. **Syncing**, on the frame: each prefab instance the file placed
//!    brought in step with its prefab, one a unit, as a scene change does.
//!
//! Components are read into the world on the frame rather than on a
//! worker: a component need not be `Send`, and the world is not shared, so
//! the second read (the first being the check) is the part left to the
//! budget.

use std::collections::HashSet;
use std::path::PathBuf;

use voltra_assets::AssetPath;
use voltra_ecs::Entity;
use voltra_jobs::{JobPool, Task};
use voltra_scene::format::{texture_paths, EntityRecord};
use voltra_scene::prefab::StagedScene;
use voltra_scene::{ComponentRegistry, Replicated, SceneError, SceneId};

mod step;
mod sync;
mod textures;

/// What a step did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Step {
    /// A unit of work: there may be more.
    Worked,
    /// Waiting on the pool; nothing more this frame.
    Waiting,
    /// Every entity is in.
    Done,
    /// The file will not load; logged, and nothing of it is left in the
    /// world.
    Failed,
}

/// What the read hands back.
#[derive(Debug)]
struct Parsed {
    staged: StagedScene,
    textures: Vec<AssetPath>,
    /// The replicated records set aside ([`Replicas::SetAside`]).
    set_aside: Vec<EntityRecord>,
}

/// What a load does with the file's replicated entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Replicas {
    /// Spawns them with the rest: no session, or the server's end.
    Spawn,
    /// Leaves them out: a client's streamed region, whose entities carry
    /// fresh ids on both ends, so the server's copy can never be its own.
    LeaveOut,
    /// Leaves them out of the world and hands them over
    /// ([`SceneBuild::take_set_aside`]): a client's level, whose server
    /// copies are dressed in them as they arrive.
    SetAside,
}

#[derive(Debug)]
enum Phase {
    Reading(Task<Result<Parsed, SceneError>>),
    Decoding {
        staged: StagedScene,
        textures: Vec<AssetPath>,
        done: usize,
    },
    Spawning(StagedScene),
    Syncing {
        entities: Vec<Entity>,
        instances: Vec<Entity>,
        done: usize,
    },
    Done(Vec<Entity>),
    Failed,
}

/// A scene file on its way into the world.
#[derive(Debug)]
pub(super) struct SceneBuild {
    path: AssetPath,
    phase: Phase,
    /// The replicated records set aside, once read, until taken.
    set_aside: Vec<EntityRecord>,
    /// The ids of the replicated entities it has spawned, as each lands:
    /// what a client lets go of from its stash when the load goes.
    replicated: Vec<SceneId>,
}

impl SceneBuild {
    /// Starts reading `path` under `root` on `jobs`. `registry` checks the
    /// components; `replicas` says what becomes of the replicated entities;
    /// `keep_ids`, when given, spawns each under its file's id
    /// ([`StagedScene::keep_ids`]) — a networked session's level — but for
    /// the ids it holds, which outlive the change (persistent entities)
    /// and are left out with what hangs under them.
    pub(super) fn start(
        jobs: &JobPool,
        root: PathBuf,
        path: &AssetPath,
        registry: ComponentRegistry,
        replicas: Replicas,
        keep_ids: Option<HashSet<SceneId>>,
    ) -> Self {
        let task_path = path.clone();
        let task = jobs.submit(move || {
            let mut staged = StagedScene::read(&root, &task_path)?;
            let replicated =
                |record: &EntityRecord| record.components.contains_key(Replicated::NAME);
            let set_aside = match replicas {
                Replicas::Spawn => Vec::new(),
                Replicas::LeaveOut => {
                    staged.skip(replicated);
                    Vec::new()
                }
                Replicas::SetAside => staged.take(replicated),
            };
            if let Some(staying) = keep_ids {
                if !staying.is_empty() {
                    staged.skip(|record| staying.contains(&record.id));
                }
                staged.keep_ids();
            }
            staged.check(&registry)?;
            let textures = texture_paths(staged.records());
            Ok(Parsed {
                staged,
                textures,
                set_aside,
            })
        });
        Self {
            path: path.clone(),
            phase: Phase::Reading(task),
            set_aside: Vec::new(),
            replicated: Vec::new(),
        }
    }

    /// The replicated records set aside ([`Replicas::SetAside`]), once the
    /// file is read; empty before, and after the first take.
    pub(super) fn take_set_aside(&mut self) -> Vec<EntityRecord> {
        std::mem::take(&mut self.set_aside)
    }

    /// The ids of the replicated entities it has spawned so far.
    pub(super) fn replicated_ids(&self) -> &[SceneId] {
        &self.replicated
    }

    /// The file it is reading.
    pub(super) fn path(&self) -> &AssetPath {
        &self.path
    }

    /// From 0 to 1: a quarter for the read, a quarter for the textures,
    /// most of the rest for the entities and the last tenth for syncing
    /// prefab instances.
    pub(super) fn progress(&self) -> f32 {
        fn part(done: usize, total: usize) -> f32 {
            if total == 0 {
                1.0
            } else {
                done as f32 / total as f32
            }
        }
        match &self.phase {
            Phase::Reading(_) | Phase::Failed => 0.0,
            Phase::Decoding { textures, done, .. } => 0.25 + 0.25 * part(*done, textures.len()),
            Phase::Spawning(staged) => 0.5 + 0.4 * part(staged.spawned(), staged.len()),
            Phase::Syncing {
                instances, done, ..
            } => 0.9 + 0.1 * part(*done, instances.len()),
            Phase::Done(_) => 1.0,
        }
    }

    /// Whether the file has been read and checked: from here on it will
    /// load (barring a component a check could not see).
    pub(super) fn is_read(&self) -> bool {
        !matches!(self.phase, Phase::Reading(_) | Phase::Failed)
    }

    /// Every entity it has put in the world so far.
    pub(super) fn entities(&self) -> &[Entity] {
        match &self.phase {
            Phase::Spawning(staged) => staged.entities(),
            Phase::Syncing { entities, .. } | Phase::Done(entities) => entities,
            Phase::Reading(_) | Phase::Decoding { .. } | Phase::Failed => &[],
        }
    }

    /// The entities it brought, once [`Step::Done`]; empty before.
    pub(super) fn into_entities(self) -> Vec<Entity> {
        match self.phase {
            Phase::Done(entities) => entities,
            _ => Vec::new(),
        }
    }
}
