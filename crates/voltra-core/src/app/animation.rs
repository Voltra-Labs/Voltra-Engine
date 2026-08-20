//! Running the scene's sprite clocks once per frame.

use voltra_scene::animation;

use crate::app::App;

impl App {
    /// Advances every playing [`SpriteAnimation`] by the frame's delta.
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
    pub(super) fn advance_animations(&mut self, delta: f32) {
        if !self.simulation.is_running() {
            return;
        }
        animation::advance(&mut self.world, delta);
    }
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
}
