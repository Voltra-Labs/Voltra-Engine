//! The draw list a 3D view turns the world into: the twin of
//! [`batch`](crate::batch) — every visible [`MeshInstance3D`], culled
//! against the view's frustum on the CPU, sorted and grouped into
//! instanced draws.
//!
//! **Order**: opaque and masked instances grouped by pipeline → material
//! → mesh, front to back within a mesh, the meshes of one pipeline and
//! material by their nearest instance — opaque draws front to back is what
//! lets the depth test reject hidden fragments early; then the blended,
//! back to front by view depth, since what blends over must come after what
//! it covers. Over [`JobPool`] when the list is long ([`PARALLEL_FROM`]).

mod cull;
mod group;
#[cfg(test)]
mod material_tests;
#[cfg(test)]
mod tests;

use std::collections::HashSet;
use std::ops::Range;

use voltra_assets::{Handle, Material, Materials, Meshes};
use voltra_ecs::{Entity, World};
use voltra_jobs::JobPool;
use voltra_render::render3d::{Instance3D, Mesh3D, View3D, Winding};

use crate::mesh_instance3d::MeshInstance3D;

/// From how many instances culling is split across a [`JobPool`].
pub const PARALLEL_FROM: usize = 4096;

/// One instanced draw: `mesh` through `material`, once for each of the
/// list's `instances` — all of one winding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draw3D {
    /// The mesh drawn.
    pub mesh: Handle<Mesh3D>,
    /// Its surface material; `None` for the default white standard.
    pub material: Option<Handle<Material>>,
    /// Which of [`DrawList3D::instances`] draw it.
    pub instances: Range<u32>,
    /// Which way their fronts wind
    /// ([`Winding::of`](voltra_render::render3d::Winding::of)): mirrored
    /// ones clockwise, flattened ones with no face culled.
    pub winding: Winding,
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
    /// The seen candidates' sort keys; kept for its storage.
    keys: Vec<group::Key>,
    /// The runs of one mesh the keys sort into; kept for its storage.
    runs: Vec<group::Run>,
    /// How many mesh instances the last rebuild culled.
    culled: usize,
    /// Entities already warned about, so a broken one is said once.
    warned: HashSet<Entity>,
    /// Entities whose material was warned about, once each.
    warned_materials: HashSet<Entity>,
}

impl DrawList3D {
    /// The draw list of `world` through `view`, its meshes from `meshes`
    /// and its materials from `materials` (`None`: every mesh the default
    /// white standard).
    pub fn from_world(
        world: &World,
        view: &View3D,
        meshes: &mut Meshes,
        materials: Option<&Materials>,
        jobs: Option<&JobPool>,
    ) -> Self {
        let mut list = Self::default();
        list.rebuild(world, view, meshes, materials, jobs);
        list
    }

    /// Rebuilds the list in place for `world` through `view`.
    pub fn rebuild(
        &mut self,
        world: &World,
        view: &View3D,
        meshes: &mut Meshes,
        materials: Option<&Materials>,
        jobs: Option<&JobPool>,
    ) {
        self.instances.clear();
        self.draws.clear();
        self.culled = 0;
        if world.components::<MeshInstance3D>().is_none() {
            self.candidates.clear();
            return;
        }
        cull::gather(
            world,
            meshes,
            materials,
            &mut self.candidates,
            &mut self.warned,
            &mut self.warned_materials,
        );
        cull::cull(&mut self.candidates, view, jobs);
        cull::report(&self.candidates, &mut self.warned);
        self.culled = self
            .candidates
            .iter()
            .filter(|c| c.instance.is_none())
            .count();
        group::group(
            &self.candidates,
            &mut self.keys,
            &mut self.runs,
            &mut self.instances,
            &mut self.draws,
        );
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
