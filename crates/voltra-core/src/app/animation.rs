//! Running the scene's sprite clocks and clip players once per frame.

use std::path::PathBuf;

use voltra_assets::AssetPath;
use voltra_scene::{animation, keyframes, AnimationClips};

use crate::app::App;

impl App {
    /// Advances every playing [`SpriteAnimation`] and [`AnimationPlayer`] by
    /// the frame's delta — the players second, so a clip that keys a frame
    /// wins over a flipbook on the same sprite.
    ///
    /// Per *frame*, not per fixed step: what is drawn is not what is
    /// simulated, and a clip on the physics clock would show two frames at
    /// once whenever a frame owed two steps and none when it owed zero.
    ///
    /// Gated on the simulation switch, like the game's tick and for the same
    /// reason: an editor that is authoring must not animate the sprite being
    /// placed. Unity draws the line in the same place — an `Animator` runs in
    /// play mode, and previewing a clip while editing is a deliberate,
    /// separate action.
    ///
    /// [`SpriteAnimation`]: voltra_scene::SpriteAnimation
    /// [`AnimationPlayer`]: voltra_scene::AnimationPlayer
    pub(super) fn advance_animations(&mut self, delta: f32) {
        if !self.simulation.is_running() {
            return;
        }
        animation::advance(&mut self.world, delta);
        let clips = clip_store(&mut self.animation_clips, &self.asset_root);
        keyframes::advance(&mut self.world, clips, delta);
    }

    /// The clips the players read, made on first use.
    pub fn animation_clips(&mut self) -> &mut AnimationClips {
        clip_store(&mut self.animation_clips, &self.asset_root)
    }

    /// Re-reads `path` if it is a clip the store has loaded. Answers whether
    /// it was one.
    pub(super) fn reload_animation_clip(&mut self, path: &AssetPath) -> bool {
        self.animation_clips
            .as_mut()
            .is_some_and(|clips| clips.reload(path))
    }
}

/// The clip store, made on first use: a clip is a file and nothing else, so
/// a headless run plays one. The root is the one the prefabs use.
pub(super) fn clip_store<'a>(
    clips: &'a mut Option<AnimationClips>,
    asset_root: &Option<PathBuf>,
) -> &'a mut AnimationClips {
    clips.get_or_insert_with(|| {
        AnimationClips::new(
            asset_root
                .clone()
                .unwrap_or_else(voltra_assets::default_root),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_ecs::Entity;
    use voltra_scene::{Sprite, SpriteAnimation};

    /// An app holding one animated sprite, with no window.
    ///
    /// `App::default` never touches the event loop, so a clock that writes a
    /// component is reachable without a device or a surface.
    fn app_with_a_clip() -> (App, Entity) {
        let mut app = App::default();
        let entity = app.world.spawn();
        app.world.insert(entity, Sprite::default());
        app.world
            .insert(entity, SpriteAnimation::new(vec![0, 1, 2, 3], 4.0));
        (app, entity)
    }

    fn frame_of(app: &App, entity: Entity) -> u32 {
        app.world
            .get::<Sprite>(entity)
            .expect("the sprite is there")
            .frame
    }

    #[test]
    fn a_frame_does_not_animate_while_simulation_is_off() {
        let (mut app, entity) = app_with_a_clip();

        app.advance_animations(0.3);

        assert_eq!(frame_of(&app, entity), 0, "the sprite being placed holds");
    }

    #[test]
    fn a_running_frame_advances_the_clip() {
        let (mut app, entity) = app_with_a_clip();
        app.set_simulating(true);

        app.advance_animations(0.3);

        assert_eq!(frame_of(&app, entity), 1);
    }

    #[test]
    fn a_player_plays_its_clip_only_while_simulating_and_takes_a_reload() {
        use voltra_scene::{AnimationPlayer, Transform};
        let root = voltra_testkit::scratch_root();
        let write = |y: f32| {
            let text = format!(
                "(tracks: [(property: Translation, keys: [(time: 0.0, value: Vec2((0.0, {y})))])])"
            );
            std::fs::write(root.join("pose.anim.ron"), text).expect("writes");
        };
        write(3.0);
        let mut app = App {
            asset_root: Some(root.clone()),
            ..App::default()
        };
        let path = AssetPath::new("pose.anim.ron").expect("valid");
        let entity = app.world.spawn();
        app.world.insert(entity, Transform::default());
        app.world.insert(entity, AnimationPlayer::new(path.clone()));
        let y = |app: &App| app.world.get::<Transform>(entity).expect("t").translation.y;

        app.advance_animations(0.1);
        assert_eq!(y(&app), 0.0, "authoring holds still");
        app.set_simulating(true);
        app.advance_animations(0.1);
        assert_eq!(y(&app), 3.0);

        write(5.0);
        assert!(app.reload_animation_clip(&path));
        app.world
            .get_mut::<AnimationPlayer>(entity)
            .expect("p")
            .restart();
        app.advance_animations(0.1);
        assert_eq!(y(&app), 5.0);
    }
}
