//! The draw list a 3D view turns the world into: the twin of
//! [`batch`](crate::batch) — every visible [`MeshInstance3D`], culled
//! against the view's frustum on the CPU, sorted and grouped into
//! instanced draws.
//!
//! **Order**: grouped by mesh (one pipeline until materials), front to back
//! within a group, groups by their nearest instance — opaque draws front to
//! back is what lets the depth test reject hidden fragments early. Over
//! [`JobPool`] when the list is long ([`PARALLEL_FROM`]).

mod cull;
mod group;
#[cfg(test)]
mod tests;

use std::collections::HashSet;
use std::ops::Range;

use voltra_assets::{Handle, Meshes};
use voltra_ecs::{Entity, World};
use voltra_jobs::JobPool;
use voltra_render::render3d::{Instance3D, Mesh3D, View3D};

use crate::mesh_instance3d::MeshInstance3D;

/// From how many instances culling is split across a [`JobPool`].
pub const PARALLEL_FROM: usize = 4096;

/// One instanced draw: `mesh`, once for each of the list's `instances`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draw3D {
    /// The mesh drawn.
    pub mesh: Handle<Mesh3D>,
    /// Which of [`DrawList3D::instances`] draw it.
    pub instances: Range<u32>,
}

/// What a 3D view draws this frame. Kept from frame to frame and rebuilt
/// in place, so its vectors are allocated once.
#[derive(Debug, Default)]
pub struct DrawList3D {
    /// The instances drawn, in draw order.
    pub instances: Vec<Instance3D>,
    /// The draws over them, in order.
    pub draws: Vec<Draw3D>,
    /// What this frame's rebuild looked at; kept for its storage.
    candidates: Vec<cull::Candidate>,
    /// How many mesh instances the last rebuild culled.
    culled: usize,
    /// Entities already warned about, so a broken one is said once.
    warned: HashSet<Entity>,
}

impl DrawList3D {
    /// The draw list of `world` through `view`.
    pub fn from_world(
        world: &World,
        view: &View3D,
        meshes: &mut Meshes,
        jobs: Option<&JobPool>,
    ) -> Self {
        let mut list = Self::default();
        list.rebuild(world, view, meshes, jobs);
        list
    }

    /// Rebuilds the list in place for `world` through `view`.
    pub fn rebuild(
        &mut self,
        world: &World,
        view: &View3D,
        meshes: &mut Meshes,
        jobs: Option<&JobPool>,
    ) {
        self.instances.clear();
        self.draws.clear();
        self.culled = 0;
        if world.components::<MeshInstance3D>().is_none() {
            self.candidates.clear();
            return;
        }
        cull::gather(world, meshes, &mut self.candidates, &mut self.warned);
        cull::cull(&mut self.candidates, view, jobs);
        self.culled = self
            .candidates
            .iter()
            .filter(|c| c.instance.is_none())
            .count();
        group::group(&mut self.candidates, &mut self.instances, &mut self.draws);
    }

    /// How many mesh instances the last rebuild left out: outside the
    /// view, collapsed to nothing, or broken.
    pub fn culled(&self) -> usize {
        self.culled
    }

    /// Whether there is nothing to draw.
    pub fn is_empty(&self) -> bool {
        self.draws.is_empty()
    }
}
