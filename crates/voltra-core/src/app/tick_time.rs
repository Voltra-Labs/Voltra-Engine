//! What a game's tick may ask of time: tweens and timers.

use voltra_ecs::Entity;
use voltra_scene::keyframes::{Ease, Property, Value};
use voltra_scene::{TimerId, TweenId};

use super::{App, Tick};

impl Tick<'_> {
    /// Moves `property` of `entity` to `to` over `seconds`, shaped by
    /// `ease` — Godot's `tween_property`. Chain another after it with
    /// `tick.tweens.then(id, …)`; ask `tick.tweens.finished(id)`. It stops
    /// if `entity` is despawned.
    pub fn tween(
        &mut self,
        entity: Entity,
        property: Property,
        to: Value,
        seconds: f32,
        ease: Ease,
    ) -> TweenId {
        self.tweens.tween(entity, property, to, seconds, ease)
    }

    /// A timer that fires once, `seconds` from now; `tick.timers.fired(id)`
    /// is true in the tick it does. With an `owner`, dropped if the owner is
    /// despawned first.
    pub fn after(&mut self, seconds: f32, owner: Option<Entity>) -> TimerId {
        self.timers.after(seconds, owner)
    }

    /// A timer that fires every `seconds` until cancelled or its owner is
    /// despawned.
    pub fn every(&mut self, seconds: f32, owner: Option<Entity>) -> TimerId {
        self.timers.every(seconds, owner)
    }
}

impl App {
    /// Steps the tweens and timers on the frame's time, while the game
    /// simulates: an editor authoring the scene moves nothing, as it plays
    /// no clip.
    pub(super) fn step_tweens_and_timers(&mut self, delta: f32) {
        if !self.simulation.is_running() {
            return;
        }
        self.tweens.step(&mut self.world, delta);
        self.timers.step(&self.world, delta);
    }
}
