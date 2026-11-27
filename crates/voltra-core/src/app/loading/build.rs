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

use std::path::PathBuf;

use voltra_assets::AssetPath;
use voltra_ecs::Entity;
use voltra_jobs::{JobPool, Task};
use voltra_scene::format::texture_paths;
use voltra_scene::prefab::StagedScene;
use voltra_scene::{ComponentRegistry, Replicated, SceneError};

mod sync;
mod textures;

use super::decodes::Decodes;
use crate::app::App;

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
}

impl SceneBuild {
    /// Starts reading `path` under `root` on `jobs`. `registry` checks the
    /// components; `skip_replicated` leaves out what a replication client
    /// gets from its server instead.
    pub(super) fn start(
        jobs: &JobPool,
        root: PathBuf,
        path: &AssetPath,
        registry: ComponentRegistry,
        skip_replicated: bool,
    ) -> Self {
        let task_path = path.clone();
        let task = jobs.submit(move || {
            let mut staged = StagedScene::read(&root, &task_path)?;
            if skip_replicated {
                staged.skip(|record| record.components.contains_key(Replicated::NAME));
            }
            staged.check(&registry)?;
            let textures = texture_paths(staged.records());
            Ok(Parsed { staged, textures })
        });
        Self {
            path: path.clone(),
            phase: Phase::Reading(task),
        }
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

impl App {
    /// Advances `build` by one unit, or finds it waiting.
    pub(super) fn step_build(
        &mut self,
        build: &mut SceneBuild,
        decodes: &mut Decodes,
        jobs: &JobPool,
    ) -> Step {
        match std::mem::replace(&mut build.phase, Phase::Failed) {
            Phase::Reading(mut task) => match task.poll() {
                None => {
                    build.phase = Phase::Reading(task);
                    Step::Waiting
                }
                Some(Err(error)) => {
                    log::error!("could not load scene {}: {error}", build.path.as_str());
                    Step::Failed
                }
                Some(Ok(parsed)) => {
                    let textures = self.request_decodes(parsed.textures, decodes, jobs);
                    build.phase = Phase::Decoding {
                        staged: parsed.staged,
                        textures,
                        done: 0,
                    };
                    Step::Worked
                }
            },
            Phase::Decoding {
                staged,
                textures,
                mut done,
            } => {
                while done < textures.len() && !decodes.is_pending(&textures[done]) {
                    done += 1;
                }
                let Some(path) = textures.get(done) else {
                    build.phase = Phase::Spawning(staged);
                    return Step::Worked;
                };
                let step = match decodes.poll(path) {
                    Some(decoded) => {
                        self.admit_decoded(decoded);
                        done += 1;
                        Step::Worked
                    }
                    None => Step::Waiting,
                };
                build.phase = Phase::Decoding {
                    staged,
                    textures,
                    done,
                };
                step
            }
            Phase::Spawning(mut staged) => match staged.spawn_next(&mut self.world, &self.registry)
            {
                Ok(Some(entity)) => {
                    self.resolve_entity(entity);
                    build.phase = Phase::Spawning(staged);
                    Step::Worked
                }
                Ok(None) => {
                    let entities = staged.entities().to_vec();
                    let instances = self.instance_roots(&entities);
                    build.phase = Phase::Syncing {
                        entities,
                        instances,
                        done: 0,
                    };
                    Step::Worked
                }
                Err(error) => {
                    log::error!("could not load scene {}: {error}", build.path.as_str());
                    staged.despawn(&mut self.world);
                    Step::Failed
                }
            },
            Phase::Syncing {
                entities,
                instances,
                done,
            } => {
                let Some(&root) = instances.get(done) else {
                    build.phase = Phase::Done(entities);
                    return Step::Done;
                };
                self.sync_instance(root);
                build.phase = Phase::Syncing {
                    entities,
                    instances,
                    done: done + 1,
                };
                Step::Worked
            }
            Phase::Done(entities) => {
                build.phase = Phase::Done(entities);
                Step::Done
            }
            Phase::Failed => Step::Failed,
        }
    }
}
