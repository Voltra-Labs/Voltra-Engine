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
    in_flight: VecDeque<Birth>,
    /// What the next dispatch runs: every step since the last one, merged.
    pending: Option<SimStep>,
    /// Steps planned so far, which seed each dispatch's randomness.
    steps: u32,
}

impl GpuPlan {
    /// The most of this emitter's particles that can be alive.
    pub(crate) fn alive_bound(&self) -> usize {
        self.in_flight
            .iter()
            .map(|birth| birth.count as usize)
            .sum()
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
        for birth in &mut self.in_flight {
            birth.age += dt;
        }
        self.in_flight.retain(|birth| birth.age < longest + MARGIN);

        let room = (capacity as usize).saturating_sub(self.alive_bound());
        let births = (births as usize).min(room) as u32;
        let start = self.cursor;
        if births > 0 {
            self.in_flight.push_back(Birth {
                age: 0.0,
                count: births,
                origin: if emitter.world_space {
                    matrix.transform_point2(Vec2::ZERO)
                } else {
                    Vec2::ZERO
                },
                reach: reach(emitter, matrix, longest),
            });
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

    /// A box every live particle is inside, in the world, for an emitter
    /// whose world matrix is `matrix` now — or `None` when none can be alive.
    ///
    /// Conservative: each birth's place grown by the farthest its particles
    /// can have gone in the longest lifetime at the fastest speed, plus the
    /// widest size. A world-space birth stays where it was born; a
    /// local-space one moves with the emitter, so its box is carried by
    /// today's matrix. What lets the batch leave out an emitter off screen.
    pub(crate) fn bounds(&self, world_space: bool, matrix: Mat3) -> Option<(Vec2, Vec2)> {
        let mut bounds: Option<(Vec2, Vec2)> = None;
        for birth in &self.in_flight {
            let (min, max) = if world_space {
                (
                    birth.origin - Vec2::splat(birth.reach),
                    birth.origin + Vec2::splat(birth.reach),
                )
            } else {
                let r = birth.reach;
                let corners = [
                    Vec2::new(-r, -r),
                    Vec2::new(r, -r),
                    Vec2::new(r, r),
                    Vec2::new(-r, r),
                ]
                .map(|corner| matrix.transform_point2(corner));
                (
                    corners.iter().copied().fold(Vec2::INFINITY, Vec2::min),
                    corners.iter().copied().fold(Vec2::NEG_INFINITY, Vec2::max),
                )
            };
            bounds = Some(match bounds {
                None => (min, max),
                Some((lo, hi)) => (lo.min(min), hi.max(max)),
            });
        }
        bounds
    }

    /// The dispatch owed since the last one, if any step was planned.
    pub(crate) fn take(&mut self) -> Option<SimStep> {
        self.pending.take()
    }
}

/// Particles born in one step: how long ago, how many, where, and how far
/// from there any of them can get.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Birth {
    age: f32,
    count: u32,
    origin: Vec2,
    reach: f32,
}

/// How far a particle of `emitter`, born through `matrix`, can be from where
/// it was born, at most, over `lifetime` seconds — in the space it is drawn
/// in before the draw's placement (the world for a world-space emitter).
fn reach(emitter: &ParticleEmitter, matrix: Mat3, lifetime: f32) -> f32 {
    let scale = if emitter.world_space {
        matrix
            .x_axis
            .truncate()
            .length()
            .max(matrix.y_axis.truncate().length())
    } else {
        1.0
    };
    let speed = finite_or(emitter.speed.0.abs().max(emitter.speed.1.abs()), 0.0) * scale;
    let acceleration = if emitter.acceleration.is_finite() {
        emitter.acceleration.length()
    } else {
        0.0
    };
    let size = finite_or(emitter.size.0.max(emitter.size.1), 0.0).max(0.0);
    let reach = speed * lifetime + 0.5 * acceleration * lifetime * lifetime + size * 0.5;
    // Infinite rather than NaN: a box that sees everything draws, which is
    // the safe answer for a reach that cannot be bounded.
    if reach.is_finite() {
        reach
    } else {
        f32::INFINITY
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
