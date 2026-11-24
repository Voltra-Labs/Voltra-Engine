//! Where an emitter is simulated: the CPU or the GPU.

/// Where an emitter's particles are stepped and drawn from.
///
/// The CPU is the default and the reference: exact counts, no device
/// needed, every particle readable. The GPU takes the per-particle work —
/// tens of thousands cost what a handful do — with the same parameters and
/// look; the CPU still decides how many are born and when. An adapter
/// without compute shaders runs a GPU emitter on the CPU instead.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Simulation {
    #[default]
    /// Simulated on the CPU, one particle at a time.
    Cpu,
    /// Simulated by a compute shader on the GPU.
    Gpu,
}

impl Simulation {
    /// Whether it is simulated on the CPU.
    pub fn is_cpu(&self) -> bool {
        *self == Self::Cpu
    }
}
