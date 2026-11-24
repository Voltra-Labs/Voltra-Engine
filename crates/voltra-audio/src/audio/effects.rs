//! The game's side of bus effects: describing them, moving their numbers,
//! and laying out every bus from a project file.

use std::time::Duration;

use super::Audio;
use crate::bus::{BusId, MAX_BUSES};
use crate::command::Command;
use crate::effect::{Effect, EffectDesc, Param, MAX_EFFECTS};
use crate::layout::BusSettings;
use crate::voice::SMOOTHING;

/// The rate effects are built at when there is a command channel and no
/// device — which only a test that wires its own channel can arrange.
const NO_DEVICE_RATE: u32 = 48_000;

impl Audio {
    /// Replaces `bus`'s effects with `effects`, run first to last before
    /// its volume.
    ///
    /// The chain is built here, on the calling thread, at the device's rate
    /// — every delay line and comb allocated now — and sent whole; the chain
    /// it replaces comes back to be freed in [`end_frame`](Self::end_frame).
    /// An effect whose numbers mean nothing (see [`EffectDesc::check`]) is
    /// left out with a warning rather than failing the rest, and past
    /// [`MAX_EFFECTS`] the chain is cut. Remembered with no device, and
    /// handed to one opened later.
    pub fn set_bus_effects(&mut self, bus: BusId, effects: Vec<EffectDesc>) {
        if bus.index() >= MAX_BUSES {
            return;
        }
        let mut kept = Vec::with_capacity(effects.len().min(MAX_EFFECTS));
        for effect in effects {
            match effect.check() {
                Ok(()) => kept.push(effect),
                Err(e) => log::warn!("{e}; leaving it off the bus"),
            }
        }
        if kept.len() > MAX_EFFECTS {
            log::warn!(
                "a bus holds at most {MAX_EFFECTS} effects; keeping the first {MAX_EFFECTS} of {}",
                kept.len()
            );
            kept.truncate(MAX_EFFECTS);
        }
        self.bus_effects[bus.index()] = kept;
        self.send_bus_effects(bus);
    }

    /// The effects on `bus` as last described, with every number a tick
    /// moved since — its target, not where a ramp has got to.
    pub fn bus_effects(&self, bus: BusId) -> &[EffectDesc] {
        self.bus_effects.get(bus.index()).map_or(&[], Vec::as_slice)
    }

    /// Slides `param` of the effect at `effect` in `bus`'s chain to `value`
    /// over `over` — a low-pass opening as the player surfaces, a reverb
    /// growing as they walk into a cave.
    ///
    /// `false`, sending nothing, when there is no such effect or it has no
    /// such parameter: a typo in a tick is told rather than silently lost.
    pub fn set_bus_param(
        &mut self,
        bus: BusId,
        effect: usize,
        param: Param,
        value: f32,
        over: Duration,
    ) -> bool {
        let Some(slot) = self
            .bus_effects
            .get_mut(bus.index())
            .and_then(|chain| chain.get_mut(effect))
            .and_then(|desc| desc.param_mut(param))
        else {
            return false;
        };
        *slot = if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        };
        self.send(Command::SetBusParam {
            bus,
            effect,
            param,
            value,
            over,
        });
        true
    }

    /// Sets every bus `layout` lists — registering it, its volume, its
    /// effects — and puts back to unity and no effects any bus the previous
    /// layout set and this one does not.
    ///
    /// Additive rather than a new table: buses keep their ids, so voices
    /// already playing stay on the bus they were started on, and a bus a
    /// scene or the game's own code named keeps working. Volumes slide over
    /// [`SMOOTHING`], so a layout saved while the game runs does not click.
    pub fn apply_layout(&mut self, layout: &[BusSettings]) {
        let previous = std::mem::take(&mut self.layout);
        for settings in layout {
            let bus = self.bus(&settings.name);
            self.set_bus_volume(bus, settings.volume, SMOOTHING);
            self.set_bus_effects(bus, settings.effects.clone());
            self.layout.push(bus);
        }
        for bus in previous {
            if !self.layout.contains(&bus) {
                self.set_bus_volume(bus, 1.0, SMOOTHING);
                self.set_bus_effects(bus, Vec::new());
            }
        }
    }

    /// Sends a device opened late every chain that is not empty.
    pub(super) fn replay_bus_effects(&mut self) {
        for index in 0..MAX_BUSES {
            if !self.bus_effects[index].is_empty() {
                self.send_bus_effects(BusId::from_index(index));
            }
        }
    }

    /// Builds `bus`'s described chain and sends it, if there is a mixer.
    fn send_bus_effects(&mut self, bus: BusId) {
        if self.is_silent() {
            return;
        }
        let rate = self.rate().unwrap_or(NO_DEVICE_RATE);
        let descs = std::mem::take(&mut self.bus_effects[bus.index()]);
        let effects = Effect::build_chain(&descs, rate, &mut self.buses);
        self.bus_effects[bus.index()] = descs;
        self.send(Command::SetBusEffects { bus, effects });
    }
}
