//! A build advanced by one unit: a read polled, a texture admitted, an
//! entity spawned, an instance synced.

use voltra_jobs::JobPool;
use voltra_scene::{Replicated, SceneId};

use super::super::decodes::Decodes;
use super::{Phase, SceneBuild, Step};
use crate::app::App;

impl App {
    /// Advances `build` by one unit, or finds it waiting.
    pub(in crate::app::loading) fn step_build(
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
                    build.set_aside = parsed.set_aside;
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
                    if self.world.get::<Replicated>(entity).is_some() {
                        if let Some(&id) = self.world.get::<SceneId>(entity) {
                            build.replicated.push(id);
                        }
                    }
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
