//! A game run without a window: load a scene, step it, feed it input, read
//! what happened.
//!
//! The player without its window and device, not a second loop: the frames
//! run [`App::advance`] — the game's ticks, the fixed physics steps,
//! animations, cameras, prefab sync, scene changes — in the same order the
//! window does. What needs a device is left out: textures are not uploaded,
//! nothing is drawn and the audio is the silent one. For CI, for tests of a
//! whole scene, and for the MCP server (`voltra-mcp`).

use std::fmt;

use ron::value::RawValue;
use voltra_assets::{AssetPath, Atlases, Clips, Fonts};
use voltra_ecs::World;
use voltra_scene::format::{entity_with_id, record_entity};
use voltra_scene::{ComponentRegistry, SceneError, SceneId};

use super::ui_frame::{resolve_world_atlases, resolve_world_clips};
use super::App;

/// An [`App`] stepped by hand, with no window.
pub struct Headless {
    app: App,
    frames: u64,
    /// What the engine logged since the session last handed its lines out.
    logs: crate::logging::LogCapture,
}

/// Why a headless edit could not be made.
#[derive(Debug)]
pub enum HeadlessError {
    /// No entity has this id.
    NoEntity(SceneId),
    /// No component is registered under this name.
    NoComponent(String),
    /// The value would not read as the component, or would not write.
    Scene(SceneError),
}

impl fmt::Display for HeadlessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoEntity(id) => write!(f, "no entity has the id {id:?}"),
            Self::NoComponent(name) => write!(f, "no component is registered as {name:?}"),
            Self::Scene(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for HeadlessError {}

impl From<SceneError> for HeadlessError {
    fn from(error: SceneError) -> Self {
        Self::Scene(error)
    }
}

impl Headless {
    /// Runs `app` — built as a game builds it, with [`App::new`], its
    /// registry and its ticks — without a window, simulating from the first
    /// frame. (`App::default` has no gravity: `new` is what sets it.)
    ///
    /// Does what the window's start does that needs no device: brings
    /// prefab instances in step, resolves slicings, faces and sounds, and
    /// reads the input map and the sorting layers from the asset root.
    pub fn new(mut app: App) -> Self {
        let root = app
            .asset_root
            .clone()
            .unwrap_or_else(voltra_assets::default_root);
        app.asset_root = Some(root.clone());
        app.simulation.set_running(true);
        app.sync_all_instances();
        let mut atlases = Atlases::new(root.clone());
        resolve_world_atlases(&mut app.world, &mut atlases);
        app.atlases = Some(atlases);
        app.silhouettes = Some(voltra_assets::Silhouettes::new(root.clone()));
        app.load_input_map(&root);
        app.load_sorting_layers(&root);
        let mut fonts = Fonts::new(root.clone());
        voltra_scene::text::resolve_fonts(&mut app.world, &mut fonts);
        app.fonts = Some(fonts);
        app.open_locale(&root);
        let mut clips = Clips::new(root.clone());
        resolve_world_clips(&mut app.world, &mut clips);
        app.clips = Some(clips);
        Self {
            app,
            frames: 0,
            logs: crate::logging::capture(),
        }
    }

    /// Loads `scene`, from `app`'s asset root, into `app`'s world through
    /// its registry, then runs it as [`new`](Self::new) does.
    pub fn open(mut app: App, scene: &AssetPath) -> Result<Self, SceneError> {
        let root = app
            .asset_root
            .clone()
            .unwrap_or_else(voltra_assets::default_root);
        voltra_scene::format::load(&root.join(scene.as_str()), &app.registry, &mut app.world)?;
        app.asset_root = Some(root);
        Ok(Self::new(app))
    }

    /// Runs `frames` frames of `delta` seconds each, as the window would:
    /// the ticks, the physics steps they owe, and the rest, then the end of
    /// the frame's input. A delta that is not finite, or below zero, is a
    /// frame of no time.
    pub fn step(&mut self, frames: u32, delta: f32) {
        let delta = if delta.is_finite() {
            delta.max(0.0)
        } else {
            0.0
        };
        for _ in 0..frames {
            // A frame is a step here: what a profile of a headless run reads.
            voltra_profile::end_frame();
            voltra_profile::count("entities", self.app.world.entity_count() as f64);
            let _frame = voltra_profile::scope("update");
            self.app.advance(delta);
            self.app.input.end_frame();
            self.frames += 1;
        }
    }

    /// Every line logged since the session started, or since this was last
    /// asked — the engine's own account of what it did while it ran. Empty
    /// unless [`crate::logging::init`] installed the logger.
    pub fn take_logs(&mut self) -> Vec<crate::logging::LogLine> {
        self.logs.drain()
    }

    /// Lays the UI out on a screen `size` points across, one pixel a point,
    /// so the controls take the input fed to [`input_mut`](Self::input_mut)
    /// as a window's would: the cursor in pixels, the buttons, the wheel,
    /// the keys, a pad and typing. Without one the controls are left alone.
    pub fn with_screen(mut self, size: voltra_render::glam::Vec2) -> Self {
        self.app.headless_screen = Some(size);
        self
    }

    /// The input the next frame reads: drive the mouse, the keys, a pad and
    /// the typing with its mutators. The frame's edges clear after each
    /// step, as a window's do.
    pub fn input_mut(&mut self) -> &mut crate::Input {
        &mut self.app.input
    }

    /// The controls after the last frame: what was clicked, changed or
    /// submitted, and what has the focus.
    pub fn controls(&self) -> &voltra_scene::UiControls {
        &self.app.ui_controls
    }

    /// The faces the world's texts resolved to: what measures a text's
    /// lines without drawing it.
    pub fn fonts(&self) -> Option<&Fonts> {
        self.app.fonts.as_ref()
    }

    /// The current language and its strings.
    pub fn locale(&self) -> &voltra_scene::Locale {
        &self.app.locale
    }

    /// Switches the language, as a game's options screen does; the texts
    /// take it on the next frame. Answers whether its table was read.
    pub fn set_language(&mut self, language: &str) -> bool {
        self.app.locale.set_language(language)
    }

    /// Moves the focus to `control`, as a menu opening does.
    pub fn focus(&mut self, control: Option<voltra_ecs::Entity>) {
        self.app.ui_controls.focus(control);
    }

    /// Frames run so far.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Holds `action` down, through the first of its bindings the map
    /// gives it — a key, a button, a stick pushed fully that way — so a
    /// game reads it exactly as it reads a player. Answers whether the map
    /// has the action with a binding to press.
    pub fn press(&mut self, action: &str) -> bool {
        self.drive(action, true)
    }

    /// Lets `action` go. Answers as [`press`](Self::press) does.
    pub fn release(&mut self, action: &str) -> bool {
        self.drive(action, false)
    }

    fn drive(&mut self, action: &str, down: bool) -> bool {
        let driven =
            crate::input::drive_action(&mut self.app.input, &self.app.actions, action, down);
        if !driven {
            log::warn!("no binding for the action {action:?} to press");
        }
        driven
    }

    pub fn world(&self) -> &World {
        &self.app.world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.app.world
    }

    pub fn registry(&self) -> &ComponentRegistry {
        &self.app.registry
    }

    /// The entity `id` names, as a scene file writes it — every registered
    /// component it carries, as RON.
    pub fn entity_ron(&self, id: SceneId) -> Result<String, HeadlessError> {
        let entity = entity_with_id(&self.app.world, id).ok_or(HeadlessError::NoEntity(id))?;
        let record = record_entity(&self.app.world, &self.app.registry, entity)
            .ok_or(HeadlessError::NoEntity(id))??;
        ron::ser::to_string_pretty(&record, ron::ser::PrettyConfig::default())
            .map_err(|e| HeadlessError::Scene(SceneError::Serialize(e)))
    }

    /// Sets the component registered as `name` on the entity `id` to
    /// `value`, RON as a scene file holds it — see
    /// [`ComponentRegistry::template`] for where to start.
    pub fn set_component(
        &mut self,
        id: SceneId,
        name: &str,
        value: &str,
    ) -> Result<(), HeadlessError> {
        let entity = entity_with_id(&self.app.world, id).ok_or(HeadlessError::NoEntity(id))?;
        let raw =
            RawValue::from_ron(value).map_err(|e| HeadlessError::Scene(SceneError::Parse(e)))?;
        self.app
            .registry
            .load_one(&mut self.app.world, entity, name, raw)
            .ok_or_else(|| HeadlessError::NoComponent(name.to_owned()))??;
        // A component read from text names no handles yet.
        self.app.spawned.push(entity);
        self.app.resolve_spawned();
        Ok(())
    }

    /// Replaces the running game with a fresh one made by `library`, keeping
    /// the world: written out through the old game's registry, read back
    /// through the new one's — what the editor does when a game's code is
    /// rebuilt mid-play. The old game is dropped first; its library may be
    /// closed once this returns. A scene the new types cannot read is left
    /// as it was, with the old game running, and the error answered.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn swap_game(
        &mut self,
        library: &crate::game::library::GameLibrary,
    ) -> Result<(), SceneError> {
        let mut registry = ComponentRegistry::with_defaults();
        library.register(&mut registry);
        let old = self.app.game.take();
        if let Err(error) =
            crate::game::swap::rebuild(&mut self.app.world, &self.app.registry, &registry)
        {
            self.app.game = old;
            return Err(error);
        }
        drop(old);
        self.app.registry = registry;
        self.app.game = Some(crate::game::Running::new(library.new_game()));
        // Contacts, sleep and pending events all name entities the rebuild
        // replaced; every handle is resolved again for the new ones.
        self.app.physics_world.reset();
        self.app.pending_events.clear();
        self.app.spawned.extend(self.app.world.entities());
        self.app.resolve_spawned();
        Ok(())
    }

    /// Gives the app back, for a caller done stepping it by hand.
    pub fn into_app(self) -> App {
        self.app
    }
}

mod screenshot;

pub use screenshot::{Screenshot, ScreenshotError};

#[cfg(test)]
mod tests;
