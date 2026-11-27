//! The sample platformer, as a game crate: what the editor loads and plays,
//! and what `voltra-platformer` ships.
//!
//! What the two turns are for, in the smallest game that shows it. The edge
//! — the frame the jump key went down — is read in [`Game::update`], because
//! that is the only place an edge is seen exactly once. The walker is
//! moved in [`Game::fixed_update`], because a move made per frame is scaled
//! by the frame rate. Unity and Godot split `Update`/`FixedUpdate`
//! and `_process`/`_physics_process` over the same two facts.
//!
//! The level is `scenes/platformer.ron`, authored like any scene; the game
//! finds what it acts on there by its own components ([`components`]): the
//! [`Walker`](components::Walker) the keys move — a `CharacterBody2D` that
//! pushes the level's crates — every
//! [`Coin`](components::Coin), the [`Goal`](components::Goal) and the
//! [`CoinCounter`](components::CoinCounter). `A`/`D` or the arrows walk,
//! `Space` jumps, F5 saves, F9 loads, F1 then a key rebinds the jump — and
//! keeps it for the next run — and F11 (or the HUD's button) goes
//! fullscreen and back. Escape pauses, and the pause menu opens the
//! engine's options screen ([`menu`]): display, volumes, controls.

#![warn(missing_docs)]

use voltra_assets::AssetPath;
use voltra_core::{Binding, Game, KeyCode, Tick, Touch};
use voltra_ecs::Entity;
use voltra_render::glam::Vec2;
use voltra_scene::ComponentRegistry;

pub mod camera;
pub mod coins;
pub mod components;
pub mod display;
pub mod ladder;
pub mod menu;
pub mod progress;
pub mod walker;

use components::{Goal, SaveButton};

/// The first level, under the asset root.
pub const FIRST_LEVEL: &str = "scenes/platformer.ron";

/// The game's state between two turns: made fresh for every play in the
/// editor, and again when its code is reloaded. What must outlive both —
/// the coins taken — is on the scene's [`CoinCounter`](components::CoinCounter).
#[derive(Debug, Default)]
pub struct Platformer {
    /// A jump asked for this frame, to be spent by the next step: the one
    /// piece of state the two turns share. Without it the fixed turn would
    /// have to read the key edge itself and would miss it on a frame that
    /// owes no step.
    jump: bool,
    /// F1 was pressed: the next key becomes jump.
    rebinding: bool,
    /// The pause menu or the options screen, when one is over the level.
    menu: menu::Menu,
    /// The walker was found to be no character, and that was said.
    warned_not_character: bool,
}

impl Game for Platformer {
    fn register(&self, registry: &mut ComponentRegistry) {
        components::register(registry);
    }

    fn start(&mut self, tick: &mut Tick<'_>) {
        // The display and the volumes the options screen kept; the
        // controls the loop has laid over the map already.
        voltra_core::options::Settings::apply_saved(tick);
        coins::start(tick.world);
        if walker::find(tick.world).is_none() {
            log::warn!("the scene has no Walker: nothing for the keys to move");
        }
    }

    fn update(&mut self, tick: &mut Tick<'_>) {
        if self.menu.update(tick) {
            return;
        }
        let Some(walker) = walker::find(tick.world) else {
            return;
        };
        let save_clicked = tick
            .world
            .query::<SaveButton>()
            .any(|(button, _)| tick.ui.clicked(button));
        if self.rebinding {
            if let Some(binding) = tick.input.pressed_binding() {
                tick.actions.rebind("jump", 0, binding);
                log::info!("jump is now {binding:?}");
                // Kept for the next run, in the player's folder — or the
                // page's storage, in a browser.
                if let Err(e) = tick.save_controls() {
                    log::error!("could not keep the new jump: {e}");
                }
                self.rebinding = false;
            }
        } else if tick.input.was_binding_pressed(Binding::Key(KeyCode::F1)) {
            log::info!("press a key for jump");
            self.rebinding = true;
        } else if save_clicked || tick.input.was_binding_pressed(Binding::Key(KeyCode::F5)) {
            let taken = coins::taken(tick.world);
            progress::save(tick, walker, taken);
            // Let go of the focus a click gave it, or the pad's South —
            // jump — would press it again on the next jump.
            tick.ui.focus(None);
        } else if display::asked(tick) {
            display::toggle(tick);
            tick.ui.focus(None);
        } else if tick.input.was_binding_pressed(Binding::Key(KeyCode::F9)) {
            if let Some(loaded) = progress::load(tick, walker) {
                coins::set(tick.world, loaded);
            }
        } else if tick.input.action("jump").pressed() {
            self.jump = true;
        }
        camera::follow(tick, walker);
    }

    fn fixed_update(&mut self, tick: &mut Tick<'_>) {
        let Some(walker) = walker::find(tick.world) else {
            return;
        };
        if self.menu.is_open() {
            // Paused: the walker stands where it is, whatever is held.
            self.jump = false;
            let moved = walker::halt(tick, walker);
            self.warn_unless(moved);
            return;
        }
        let worth = coins::take(tick, walker);
        if worth > 0 {
            coins::add(tick.world, worth);
        }
        if let Some(next) = reached_goal(tick, walker) {
            // The walker, camera and HUD are `Persistent` and come along;
            // everything else is replaced once this frame is over.
            match AssetPath::new(&next) {
                Ok(path) => tick.load_scene(&path),
                Err(e) => log::error!("the goal names no scene: {e}"),
            }
            walker::place(tick.world, walker, Vec2::ZERO);
        }
        // Taken, not read: a jump is spent once, whether it was asked for one
        // step ago or three.
        let jump = std::mem::take(&mut self.jump);
        walker::steer(tick, walker, jump);
        ladder::climb(tick, walker);
        let moved = walker::step(tick, walker);
        self.warn_unless(moved);
    }
}

impl Platformer {
    /// Says once, not every step, that the walker could not be moved.
    fn warn_unless(&mut self, moved: bool) {
        if !moved && !self.warned_not_character {
            log::warn!(
                "the walker needs a CharacterBody2D, a Collider2D and a Transform2D to move"
            );
        }
        self.warned_not_character |= !moved;
    }
}

/// The scene the goal `walker` began touching this step leads to.
fn reached_goal(tick: &Tick<'_>, walker: Entity) -> Option<String> {
    tick.events
        .iter()
        .filter(|event| event.touch == Touch::Began)
        .filter_map(|event| event.other(walker))
        .find_map(|other| tick.world.get::<Goal>(other).map(|goal| goal.next.clone()))
}

voltra_core::export_game!(Platformer::default());
