//! A frame's passes and the textures between them, declared, then run.
//!
//! The shape of Bevy's render graph and Unreal's RDG, cut to what a 2D
//! frame needs: passes in the order they are added, each naming the slots it
//! reads and writes. A slot is a texture the caller **imports** (the scene
//! target, the swapchain view) or one the graph **creates** for the frame.
//! Running it:
//!
//! 1. **Validates** — a pass may only read a slot imported or written by an
//!    earlier pass; anything else is a [`GraphError`], not a black frame.
//! 2. **Culls** — a pass contributing to no imported output is dropped, so
//!    an effect turned off costs nothing however it was wired.
//! 3. **Allocates** created slots from a [`TexturePool`] kept across frames:
//!    a texture is reused once its last reader in the frame has run, and
//!    next frame the same descriptions find the same textures.
//! 4. **Records** each surviving pass into the caller's encoder.

mod pool;

use std::fmt;

pub use pool::{TextureDesc, TexturePool};

/// A texture a pass reads or writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Slot(usize);

/// What a pass is handed when it records: the view behind each slot.
pub struct Views<'v> {
    views: Vec<Option<&'v wgpu::TextureView>>,
}

impl Views<'_> {
    /// The view behind `slot`.
    ///
    /// Only the slots the pass declared are guaranteed; the graph resolves
    /// those before calling it.
    pub fn get(&self, slot: Slot) -> &wgpu::TextureView {
        self.views[slot.0].expect("a pass only asks for the slots it declared, which are resolved")
    }
}

type Record<'a> = Box<dyn FnOnce(&mut wgpu::CommandEncoder, &Views<'_>) + 'a>;

enum Resource<'a> {
    Imported {
        view: &'a wgpu::TextureView,
        output: bool,
    },
    Created(TextureDesc),
}

struct Pass<'a> {
    name: &'static str,
    reads: Vec<Slot>,
    writes: Vec<Slot>,
    record: Record<'a>,
}

/// A graph that cannot run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    /// `pass` reads a slot nothing imported and no earlier pass wrote.
    ReadBeforeWrite {
        /// The name of the pass that reads it.
        pass: &'static str,
    },
    /// `pass` names a slot this graph never handed out.
    UnknownSlot {
        /// The name of the pass that names it.
        pass: &'static str,
    },
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadBeforeWrite { pass } => {
                write!(
                    f,
                    "render pass `{pass}` reads a texture nothing has written"
                )
            }
            Self::UnknownSlot { pass } => {
                write!(f, "render pass `{pass}` names an unknown texture")
            }
        }
    }
}

impl std::error::Error for GraphError {}

/// One frame's passes, built and then consumed by [`RenderGraph::run`].
#[derive(Default)]
pub struct RenderGraph<'a> {
    resources: Vec<Resource<'a>>,
    passes: Vec<Pass<'a>>,
    /// Times each pass on the GPU by its name, when given one.
    timer: Option<&'a mut crate::gpu_timer::GpuTimer>,
}

impl<'a> RenderGraph<'a> {
    /// An empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Times every pass that runs on the GPU, under its name.
    pub fn time_with(&mut self, timer: &'a mut crate::gpu_timer::GpuTimer) {
        self.timer = Some(timer);
    }

    /// A texture the caller owns and has already filled, to be read.
    pub fn import(&mut self, view: &'a wgpu::TextureView) -> Slot {
        self.push(Resource::Imported {
            view,
            output: false,
        })
    }

    /// A texture the caller owns and wants written: the passes that lead to
    /// it are the ones that run.
    pub fn import_output(&mut self, view: &'a wgpu::TextureView) -> Slot {
        self.push(Resource::Imported { view, output: true })
    }

    /// A texture that lives for this frame only, made or reused from the
    /// pool when the graph runs.
    pub fn create(&mut self, desc: TextureDesc) -> Slot {
        self.push(Resource::Created(desc))
    }

    /// Adds a pass, run after every pass added before it.
    pub fn add_pass(
        &mut self,
        name: &'static str,
        reads: &[Slot],
        writes: &[Slot],
        record: impl FnOnce(&mut wgpu::CommandEncoder, &Views<'_>) + 'a,
    ) {
        self.passes.push(Pass {
            name,
            reads: reads.to_vec(),
            writes: writes.to_vec(),
            record: Box::new(record),
        });
    }

    /// How many passes would run: what culling leaves, for a test or a
    /// profiler. Errors as [`RenderGraph::run`] would.
    pub fn live_passes(&self) -> Result<usize, GraphError> {
        self.validate()?;
        Ok(self.live().iter().filter(|&&live| live).count())
    }

    /// Validates, culls, allocates and records every surviving pass into
    /// `encoder`. Nothing is recorded if the graph is invalid.
    pub fn run(
        self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        pool: &mut TexturePool,
    ) -> Result<(), GraphError> {
        self.validate()?;
        let live = self.live();

        // Where each created slot is last read, so its texture can go back
        // to the pool for a later pass of the same frame.
        let mut last_use = vec![None; self.resources.len()];
        for (index, pass) in self.passes.iter().enumerate().filter(|(i, _)| live[*i]) {
            for slot in pass.reads.iter().chain(&pass.writes) {
                last_use[slot.0] = Some(index);
            }
        }

        pool.begin_frame();
        let mut assigned: Vec<Option<usize>> = vec![None; self.resources.len()];
        let mut order = Vec::new();
        for (index, pass) in self.passes.iter().enumerate().filter(|(i, _)| live[*i]) {
            for slot in pass.reads.iter().chain(&pass.writes) {
                if let (Resource::Created(desc), None) = (&self.resources[slot.0], assigned[slot.0])
                {
                    assigned[slot.0] = Some(pool.acquire(device, desc));
                }
            }
            // Handed back once this pass, its last user, has run; recorded
            // in pass order, so a later pass reusing it writes after.
            let done: Vec<usize> = self
                .resources
                .iter()
                .enumerate()
                .filter(|&(slot, _)| last_use[slot] == Some(index))
                .filter_map(|(slot, _)| assigned[slot])
                .collect();
            // Free for the passes after this one; recording keeps pass
            // order, so a reuse always writes after the last read.
            for &entry in &done {
                pool.release(entry);
            }
            order.push(index);
        }

        let mut timer = self.timer;
        let mut passes: Vec<Option<Pass<'a>>> = self.passes.into_iter().map(Some).collect();
        for index in order {
            let pass = passes[index].take().expect("each pass runs once");
            {
                let views = Views {
                    views: self
                        .resources
                        .iter()
                        .enumerate()
                        .map(|(slot, resource)| match resource {
                            Resource::Imported { view, .. } => Some(*view),
                            Resource::Created(_) => assigned[slot].map(|entry| pool.view(entry)),
                        })
                        .collect(),
                };
                let scope = timer.as_mut().map(|timer| timer.begin(encoder, pass.name));
                (pass.record)(encoder, &views);
                if let (Some(timer), Some(scope)) = (timer.as_mut(), scope) {
                    timer.end(encoder, scope);
                }
            }
        }
        pool.end_frame();
        Ok(())
    }

    fn push(&mut self, resource: Resource<'a>) -> Slot {
        self.resources.push(resource);
        Slot(self.resources.len() - 1)
    }

    fn validate(&self) -> Result<(), GraphError> {
        let mut written: Vec<bool> = self
            .resources
            .iter()
            .map(|resource| matches!(resource, Resource::Imported { .. }))
            .collect();
        for pass in &self.passes {
            for slot in pass.reads.iter().chain(&pass.writes) {
                if slot.0 >= self.resources.len() {
                    return Err(GraphError::UnknownSlot { pass: pass.name });
                }
            }
            if pass.reads.iter().any(|slot| !written[slot.0]) {
                return Err(GraphError::ReadBeforeWrite { pass: pass.name });
            }
            for slot in &pass.writes {
                written[slot.0] = true;
            }
        }
        Ok(())
    }

    /// Which passes lead to an output, walked from the last pass back.
    fn live(&self) -> Vec<bool> {
        let mut needed: Vec<bool> = self
            .resources
            .iter()
            .map(|resource| matches!(resource, Resource::Imported { output: true, .. }))
            .collect();
        let mut live = vec![false; self.passes.len()];
        for (index, pass) in self.passes.iter().enumerate().rev() {
            if pass.writes.iter().any(|slot| needed[slot.0]) {
                live[index] = true;
                for slot in &pass.reads {
                    needed[slot.0] = true;
                }
            }
        }
        live
    }
}

#[cfg(test)]
mod tests;
