//! What the CPU keeps for an emitter simulated on the GPU: how many are
//! born each step, which ring slots they take, and a conservative count of
//! how many may still be alive — so births wait for room as they do on the
//! CPU, and a live particle's slot is never handed out again.

use std::collections::VecDeque;

use voltra_render::glam::{Mat3, Vec2};
use voltra_render::particles::MAX_GPU_PARTICLES;
use voltra_render::{ParticleLook, SimStep};

use super::ParticleEmitter;

/// How long past its longest lifetime a birth is still counted alive.
///
/// The GPU ages a newborn from the dispatch after the step it was born in,
/// so it can lag the CPU's count by up to a frame; counting it a little
/// longer only ever delays a birth, never overwrites a live particle.
const MARGIN: f32 = 0.1;

/// An emitter's GPU-side ledger. Inactive — capacity zero — while the
/// emitter runs on the CPU.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GpuPlan {
    /// Ring slots; what the renderer's buffer must hold.
    pub(crate) capacity: u32,
    /// Where the next birth goes.
    cursor: u32,
    /// Births not yet known dead: seconds since, and how many.
    in_flight: VecDeque<(f32, u32)>,
    /// What the next dispatch runs: every step since the last one, merged.
    pending: Option<SimStep>,
    /// Steps planned so far, which seed each dispatch's randomness.
    steps: u32,
}

impl GpuPlan {
    /// The most of this emitter's particles that can be alive.
    pub(crate) fn alive_bound(&self) -> usize {
        self.in_flight.iter().map(|&(_, n)| n as usize).sum()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.in_flight.is_empty()
    }

    /// Plans one step of `dt` with `births` asked for, the emitter at
    /// `matrix`. Returns how many were actually born.
    pub(crate) fn step(
        &mut self,
        emitter: &ParticleEmitter,
        matrix: Mat3,
        dt: f32,
        births: u32,
    ) -> u32 {
        let capacity = emitter.max_particles.clamp(1, MAX_GPU_PARTICLES);
        if capacity != self.capacity {
            // A new ring: whatever the old one held is gone with it.
            *self = Self {
                capacity,
                ..Self::default()
            };
        }

        let longest = finite_or(emitter.lifetime.0.max(emitter.lifetime.1), 0.0).max(0.0);
        for (age, _) in &mut self.in_flight {
            *age += dt;
        }
        self.in_flight.retain(|&(age, _)| age < longest + MARGIN);

        let room = (capacity as usize).saturating_sub(self.alive_bound());
        let births = (births as usize).min(room) as u32;
        let start = self.cursor;
        if births > 0 {
            self.in_flight.push_back((0.0, births));
            self.cursor =
                ((u64::from(self.cursor) + u64::from(births)) % u64::from(capacity)) as u32;
        }

        let (origin, basis) = if emitter.world_space {
            (
                matrix.transform_point2(Vec2::ZERO),
                [
                    matrix.transform_vector2(Vec2::X),
                    matrix.transform_vector2(Vec2::Y),
                ],
            )
        } else {
            (Vec2::ZERO, [Vec2::X, Vec2::Y])
        };
        let latest = SimStep {
            dt,
            spawn_start: start,
            spawn_count: births,
            seed: self.steps,
            origin,
            basis,
            direction: emitter.direction,
            spread: emitter.spread,
            speed: emitter.speed,
            lifetime: emitter.lifetime,
            acceleration: emitter.acceleration,
        };
        self.steps = self.steps.wrapping_add(1);
        self.pending = Some(match self.pending.take() {
            None => latest,
            // Several steps before a frame is drawn: one dispatch moves the
            // live ones the whole way and births every step's newborns.
            Some(earlier) => SimStep {
                dt: earlier.dt + dt,
                spawn_start: if earlier.spawn_count == 0 {
                    start
                } else {
                    earlier.spawn_start
                },
                spawn_count: (earlier.spawn_count + births).min(capacity),
                seed: earlier.seed,
                ..latest
            },
        });
        births
    }

    /// The dispatch owed since the last one, if any step was planned.
    pub(crate) fn take(&mut self) -> Option<SimStep> {
        self.pending.take()
    }
}

/// How an emitter's GPU particles are drawn, placed by its world `matrix`.
pub(crate) fn look(emitter: &ParticleEmitter, matrix: Mat3) -> ParticleLook {
    ParticleLook {
        place: if emitter.world_space {
            Mat3::IDENTITY
        } else {
            matrix
        },
        size: emitter.size,
        color_start: emitter.color_start,
        color_end: emitter.color_end,
    }
}

fn finite_or(v: f32, fallback: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        fallback
    }
}
