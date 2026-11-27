//! What a game is handed when the loop gives it a turn.
//!
//! The editor gets [`UiFrame`](super::UiFrame); a game gets this. Both exist
//! for the same reason: the callback needs several of `App`'s fields at once,
//! and handing out `&mut App` would make every field of the platform layer
//! part of the engine's public surface.
//!
//! There is one context type for both hooks. Unity's `Update` and
//! `FixedUpdate` take no arguments and read `Time.deltaTime` or
//! `Time.fixedDeltaTime`; Godot's `_process(delta)` and
//! `_physics_process(delta)` take the one that applies. The second shape is
//! the better one — a tick that is told its own step cannot read the wrong
//! clock — and once `delta` is a field, the two hooks want exactly the same
//! things and a second type would differ only in its name.

use voltra_ecs::{Entity, World};
use voltra_physics::{CollisionEvent, Contact, JointBroken};
use voltra_render::glam::Vec2;
use voltra_scene::{ComponentRegistry, Prefabs};

use crate::app::tick_audio::TickAudio;
use crate::app::App;
use crate::game::Turn;
use crate::input::{ActionMap, Input};

/// A game's turn at the world.
///
/// Fields rather than accessors: this is a bag of borrows with no invariant to
/// protect, and a game reaching for `tick.world` should not have to go through
/// a method that does nothing.
pub struct Tick<'a> {
    /// The scene, to read and to change. Spawning and despawning are both fine
    /// here; nothing downstream in the frame holds an entity across the call.
    pub world: &'a mut World,
    /// Keyboard and mouse for this frame.
    ///
    /// Edge-triggered reads — [`Input::was_key_pressed`] and its relatives —
    /// are per *frame*, not per fixed step. A fixed tick that runs twice in
    /// one frame therefore sees the same press twice, and one that runs zero
    /// times misses it. Unity's manual says the same thing about reading input
    /// in `FixedUpdate`: hold-state belongs in the fixed tick, edges belong in
    /// the per-frame one.
    pub input: &'a Input,
    /// Seconds this tick covers: the frame's delta in the per-frame hook, the
    /// fixed step in the fixed one.
    pub delta: f32,
    /// The world's gravity, in world units per second squared: what the
    /// physics step pulls bodies by ([`App::gravity`]), for a game moving a
    /// character itself to fall by the same. Godot's `get_gravity()`.
    pub gravity: Vec2,
    /// What the last physics step found overlapping.
    ///
    /// State, not a stream: the same resting contact is here on every step it
    /// lasts. Empty until something steps, and sensors are never in it.
    pub contacts: &'a [Contact],
    /// What began and ended touching, sensors included.
    ///
    /// A stream, unlike [`Tick::contacts`]: each hook is handed every event
    /// exactly once. The fixed hook gets the step it is about to follow's
    /// predecessor; the per-frame hook gets everything since its last turn,
    /// however many steps that was. Neither can miss a pickup, and neither can
    /// take it twice.
    ///
    /// `a` is always the lower entity of the pair, so
    /// [`CollisionEvent::other`] is how a game asks what *it* touched.
    pub events: &'a [CollisionEvent],
    /// The joints that broke under their `break_force` or `break_torque`,
    /// handed out as [`Tick::events`] are: each once, to each hook. The
    /// `Joint2D` is already gone from the entity.
    pub broken_joints: &'a [JointBroken],
    /// The sound, for the one-shot a tick is usually reacting with.
    ///
    /// A field like the rest, but not a bare `&mut Audio`: playing takes a
    /// handle, and resolving one takes the store — see [`TickAudio`].
    pub audio: TickAudio<'a>,
    /// The game's input map, to rebind from — "press a key for jump" is
    /// [`Input::pressed_binding`] into [`ActionMap::rebind`]. A change is
    /// read from the next frame on; [`save_controls`](Self::save_controls)
    /// keeps it for the player's next run, on every platform.
    pub actions: &'a mut ActionMap,
    /// The game's save slots. Reads and writes happen on the spot: a save is
    /// a small file, and a game saving at a checkpoint wants it written
    /// before anything else can go wrong.
    pub saves: &'a crate::saves::Saves,
    /// The current language and its strings: switch it with
    /// `set_language`, read a string built in code with `text`. Texts
    /// naming a key follow it from the next frame.
    pub locale: &'a mut voltra_scene::Locale,
    /// Properties moving to values over time: start one with
    /// [`tween`](Self::tween), chain with `tweens.then`, ask
    /// `tweens.finished(id)`.
    pub tweens: &'a mut voltra_scene::Tweens,
    /// Moments to come: [`after`](Self::after) and [`every`](Self::every)
    /// make them, `timers.fired(id)` says when one came.
    pub timers: &'a mut voltra_scene::Timers,
    /// The screen-space controls: what was clicked, changed or submitted
    /// this frame, what is hovered and focused, and where to move the
    /// focus.
    pub ui: &'a mut voltra_scene::UiControls,
    /// The network session: host or join one, send to peers, read what
    /// they sent and who came and went this frame — see [`Net`].
    ///
    /// [`Net`]: super::Net
    pub net: &'a mut super::Net,
    /// The window: fullscreen, its size, vsync and the monitors' modes,
    /// read as the loop last saw them and changed from the next frame on.
    pub window: &'a mut crate::window::GameWindow,
    /// The actions as the game ships them, which a save keeps the
    /// difference from.
    pub(super) default_actions: &'a ActionMap,
    // Reached through `Tick::prefab` and `Tick::spawn`, which keep the
    // three in step: a spawn that was not queued would never be resolved.
    pub(super) scenes: &'a mut super::scenes::Scenes,
    pub(super) prefabs: &'a mut Prefabs,
    pub(super) registry: &'a ComponentRegistry,
    pub(super) spawned: &'a mut Vec<Entity>,
}

/// A hook the game installs on the loop.
pub(super) type TickFn = Box<dyn FnMut(&mut Tick<'_>)>;

impl App {
    /// Runs the per-frame tick, if there is one and the world is live.
    ///
    /// Gated on the simulation switch for the same reason physics is: while an
    /// editor is authoring, a game's logic moving the entity being placed is
    /// the scene editing itself. Unity draws the line in the same place —
    /// scripts run in play mode, and `[ExecuteInEditMode]` is the opt-in
    /// exception rather than the rule.
    pub(super) fn run_update(&mut self, delta: f32) {
        if !self.simulation.is_running() {
            return;
        }
        self.take_turn(Turn::Frame, delta);
    }

    /// Runs the fixed tick, if there is one. Called before each step.
    ///
    /// Not gated on the switch: the caller is the step loop, and a step that
    /// runs — including the single one an editor's Step button asks for while
    /// paused — is a step the game's logic belongs in front of. "Advance the
    /// world by one step" has to mean the whole world.
    pub(super) fn run_fixed_update(&mut self, delta: f32) {
        self.take_turn(Turn::Fixed, delta);
    }

    /// One turn: the hook installed for it, then the game's.
    ///
    /// Both are taken out of the app while they run, so the tick can borrow
    /// every other field, and put back after.
    fn take_turn(&mut self, turn: Turn, delta: f32) {
        let mut hook = match turn {
            Turn::Frame => self.update.take(),
            Turn::Fixed => self.fixed_update.take(),
        };
        let mut game = self.game.take();
        if hook.is_some() || game.is_some() {
            let (events, broken_joints): (&[CollisionEvent], &[JointBroken]) = match turn {
                Turn::Frame => (
                    &self.pending_physics.collisions,
                    &self.pending_physics.broken,
                ),
                // The step before this one's: the step this tick precedes has
                // not run, so there is nothing newer to hand it.
                Turn::Fixed => (
                    self.physics_world.events(),
                    self.physics_world.broken_joints(),
                ),
            };
            let mut tick = Tick {
                world: &mut self.world,
                input: &self.input,
                actions: &mut self.actions,
                default_actions: &self.default_actions,
                scenes: &mut self.scenes,
                saves: &self.saves,
                locale: &mut self.locale,
                tweens: &mut self.tweens,
                timers: &mut self.timers,
                ui: &mut self.ui_controls,
                net: &mut self.net,
                window: &mut self.game_window,
                delta,
                gravity: self.gravity,
                contacts: self.physics_world.contacts(),
                events,
                broken_joints,
                audio: TickAudio::new(&mut self.audio, self.clips.as_ref()),
                prefabs: super::spawning::prefab_store(
                    &mut self.prefabs,
                    &self.asset_root,
                    &self.registry,
                ),
                registry: &self.registry,
                spawned: &mut self.spawned,
            };
            if let Some(hook) = hook.as_mut() {
                hook(&mut tick);
            }
            if let Some(game) = game.as_mut() {
                game.take_turn(turn, &mut tick);
            }
        }
        match turn {
            Turn::Frame => self.update = hook,
            Turn::Fixed => self.fixed_update = hook,
        }
        self.game = game;
    }
}

#[cfg(test)]
mod tests;
