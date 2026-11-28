//! Each drawn instance given the lights that matter most to it.

use std::sync::atomic::{AtomicUsize, Ordering};

use voltra_jobs::JobPool;
use voltra_render::render3d::{Instance3D, LIGHTS_PER_INSTANCE};

use super::DrawList3D;
use crate::lighting3d::{pick, Gathered3D};

/// From how many instance-light pairs choosing is split across a
/// [`JobPool`]: some 65 000 estimates, a few hundred microseconds alone.
pub const LIGHT_PARALLEL_FROM: usize = 1 << 16;

/// What choosing the instances' lights found over the cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Crowded3D {
    /// Instances reached by more than
    /// [`LIGHTS_PER_INSTANCE`] point and spot lights: lit by the eight
    /// that matter most, the rest dropped.
    pub instances: usize,
    /// The most point and spot lights reaching one instance.
    pub most: usize,
}

impl DrawList3D {
    /// Gives each instance of the list its lights from `gathered` — the
    /// directional ones on its layers, the point and spot lights
    /// estimated to light it most ([`pick`]) — across `jobs` when the
    /// pairs are many. What it dropped past the eight a mesh, counted.
    pub fn light(&mut self, gathered: &Gathered3D, jobs: Option<&JobPool>) -> Crowded3D {
        let crowded = AtomicUsize::new(0);
        let most = AtomicUsize::new(0);
        let reach = &self.reach;
        let one = |at: usize, instances: &mut [Instance3D]| {
            for (offset, instance) in instances.iter_mut().enumerate() {
                let Some((bounds, layers)) = reach.get(at + offset) else {
                    continue;
                };
                let picked = pick(
                    &gathered.punctual,
                    &gathered.directional_layers,
                    bounds,
                    *layers,
                );
                instance.set_lights(&picked.list);
                if picked.reached > LIGHTS_PER_INSTANCE {
                    crowded.fetch_add(1, Ordering::Relaxed);
                    most.fetch_max(picked.reached, Ordering::Relaxed);
                }
            }
        };
        let pairs = self.instances.len() * gathered.punctual.len().max(1);
        match jobs {
            Some(jobs) if pairs >= LIGHT_PARALLEL_FROM => {
                jobs.for_each_chunk_mut(&mut self.instances, 256, one);
            }
            _ => one(0, &mut self.instances),
        }
        Crowded3D {
            instances: crowded.into_inner(),
            most: most.into_inner(),
        }
    }
}
