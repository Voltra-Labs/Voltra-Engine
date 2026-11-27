//! The pause menu — Escape or a pad's Start — and the engine's options
//! screen it opens. While either shows, the level stands still under it.

use voltra_assets::AssetPath;
use voltra_core::options::{OptionsConfig, OptionsScreen, Outcome};
use voltra_core::Tick;
use voltra_ecs::Entity;
use voltra_scene::{hierarchy, Name, PrefabInstance, PrefabPart, Transform2D};

/// The pause menu's prefab, under the asset root.
pub const PAUSE_PREFAB: &str = "prefabs/pause.prefab.ron";

/// The actions the options screen offers to rebind, as it names them.
pub const REBINDABLE: [(&str, &str); 3] = [
    ("jump", "Jump"),
    ("move_left", "Move left"),
    ("move_right", "Move right"),
];

/// What is over the level.
#[derive(Debug, Default)]
pub enum Menu {
    /// Nothing: the game plays.
    #[default]
    Closed,
    /// The pause menu.
    Paused(PauseMenu),
    /// The options screen, opened from the pause menu. Boxed: it is most of
    /// the enum's size, and the level spends its life `Closed`.
    Options(Box<OptionsScreen>),
}

/// The pause menu's entities.
#[derive(Debug)]
pub struct PauseMenu {
    root: Entity,
    resume: Option<Entity>,
    options: Option<Entity>,
}

impl Menu {
    /// Whether the level is paused under a menu.
    pub fn is_open(&self) -> bool {
        !matches!(self, Self::Closed)
    }

    /// One frame of whichever menu shows, or of none. Answers whether the
    /// game should leave this frame alone: a menu showing, or one that
    /// closed on this frame's key.
    pub fn update(&mut self, tick: &mut Tick<'_>) -> bool {
        let was_open = self.is_open();
        *self = match std::mem::take(self) {
            Self::Closed if tick.input.action("pause").pressed() => open_pause(tick),
            Self::Closed => Self::Closed,
            Self::Paused(pause) => pause.update(tick),
            Self::Options(mut screen) => match screen.update(tick) {
                Outcome::Open => Self::Options(screen),
                // Back where it was opened from.
                Outcome::Closed => open_pause(tick),
            },
        };
        was_open || self.is_open()
    }
}

impl PauseMenu {
    fn update(self, tick: &mut Tick<'_>) -> Menu {
        let clicked = |control: Option<Entity>| control.is_some_and(|e| tick.ui.clicked(e));
        let to_options = clicked(self.options);
        let resume = clicked(self.resume)
            || tick.input.action("pause").pressed()
            || tick.input.action("ui_cancel").pressed();
        if to_options {
            self.close(tick);
            let config = OptionsConfig::with_actions(&REBINDABLE);
            return match OptionsScreen::open(tick, &config) {
                Ok(screen) => Menu::Options(Box::new(screen)),
                Err(e) => {
                    log::error!("{e}");
                    open_pause(tick)
                }
            };
        }
        if resume {
            self.close(tick);
            return Menu::Closed;
        }
        Menu::Paused(self)
    }

    fn close(&self, tick: &mut Tick<'_>) {
        if tick.world.is_alive(self.root) {
            hierarchy::despawn_recursive(tick.world, self.root);
        }
        tick.ui.focus(None);
    }
}

/// Spawns the pause menu and gives it the focus; with no menu to spawn,
/// the game plays on, and the log says why.
fn open_pause(tick: &mut Tick<'_>) -> Menu {
    let path = match AssetPath::new(PAUSE_PREFAB) {
        Ok(path) => path,
        Err(e) => {
            log::error!("the pause menu's path: {e}");
            return Menu::Closed;
        }
    };
    let prefab = tick.prefab(&path);
    let instance = match tick.spawn(prefab, &Transform2D::default()) {
        Ok(instance) => instance,
        Err(e) => {
            log::error!("the pause menu would not spawn: {e}");
            return Menu::Closed;
        }
    };
    let Some(root) = instance.root() else {
        log::error!("{PAUSE_PREFAB} holds no menu");
        return Menu::Closed;
    };
    // A menu the code opens and closes is not kept in step with its file.
    for &entity in &instance.entities {
        tick.world.remove::<PrefabInstance>(entity);
        tick.world.remove::<PrefabPart>(entity);
    }
    let find = |name: &str| {
        instance.entities.iter().copied().find(|&e| {
            tick.world
                .get::<Name>(e)
                .is_some_and(|n| n.as_str() == name)
        })
    };
    let menu = PauseMenu {
        root,
        resume: find("Resume button"),
        options: find("Options button"),
    };
    tick.ui.focus(menu.resume);
    Menu::Paused(menu)
}
