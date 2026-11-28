//! One frame's turn of the loop: input, ticks, physics, audio and the
//! network, before the draw.

use super::App;

impl App {
    /// One simulation step, between the events arriving and `Input::end_frame`
    /// wiping the per-frame sets.
    ///
    /// Deliberately empty of camera *navigation*. How a scene is flown around
    /// is the editor's business, not the platform layer's; `voltra-editor`
    /// does it from the viewport panel. A game reads [`Input`](crate::Input) and moves its
    /// own camera — or authors a [`Camera2D`] into the scene, which the no-UI
    /// draw path frames through.
    ///
    /// [`Camera2D`]: voltra_scene::Camera2D
    pub(super) fn update(&mut self) {
        // The frame before this one ends here, its draw and present included.
        voltra_profile::end_frame();
        voltra_profile::count("entities", self.world.entity_count() as f64);
        let _update = voltra_profile::scope("update");
        // Before the frame's actions are derived, so a pad press is seen the
        // frame it happened, like a key's.
        if let Some(pads) = self.gamepads.as_mut() {
            pads.poll(&mut self.input);
        }
        // With the devices: a held action reads as a key held this frame.
        self.held_actions.tick(&mut self.input, &self.actions);
        let delta = self.clock.tick();
        self.elapsed += delta.as_secs_f64();
        self.advance(delta.as_secs_f32());
        self.reload_changed_assets();
    }

    /// One frame of world time: the game's tick, then the steps it owes.
    ///
    /// Separated from [`App::update`] so a test can hand it a delta instead of
    /// waiting for one.
    ///
    /// **The game's tick runs before the steps, not after.** Unity's order is
    /// the other way round — `FixedUpdate`, then `Update` — which costs a
    /// velocity set from input one step of latency before it is integrated.
    /// Nothing here has to inherit that: input is read at the top of the frame,
    /// so the frame that saw the key is the frame that moves.
    pub(crate) fn advance(&mut self, delta: f32) {
        // Once per frame, before anything reads an action: the fixed tick
        // below sees the same actions as the per-frame one.
        let input = voltra_profile::scope("input");
        // The size touch zones divide, before the actions read them.
        if let Some(window) = self.window.as_ref() {
            let size = window.inner_size();
            self.input.set_screen(voltra_render::glam::Vec2::new(
                size.width as f32,
                size.height as f32,
            ));
        } else if let Some(screen) = self.headless_screen {
            self.input.set_screen(screen);
        }
        self.input.update_actions(&self.actions);
        // After the actions, which navigate the focus, and before the tick,
        // which reads this frame's clicks.
        self.update_ui_controls(delta);
        // A theme the controls just applied may name skins and fonts.
        self.resolve_restyled();
        drop(input);
        // Before the tick, so it reads this frame's messages and events.
        self.receive_net(delta);
        // Before the tick too, which reads what the server sent: spawned,
        // despawned and changed.
        self.receive_replication();
        // Before the tick, so it reads this frame's finished tweens and
        // fired timers, as it reads this frame's clicks.
        self.step_tweens_and_timers(delta);
        {
            let _tick = voltra_profile::scope("tick");
            self.run_update(delta);
        }
        // After the tick, which may have switched the language or spawned a
        // keyed text, and before anything draws one.
        self.locale.localize(&mut self.world);
        if let Some(fonts) = self.fonts.as_mut() {
            self.locale.apply_fonts(fonts);
        }
        // Before anything else looks at what the tick made: a spawned sprite
        // is drawn, and a spawned source started, with its handles in place.
        self.resolve_spawned();
        // After the tick, so a clip the game just switched to is on screen this
        // frame rather than next.
        {
            let _animation = voltra_profile::scope("animation");
            self.advance_animations(delta);
            self.update_camera_rigs(delta);
        }
        {
            let _particles = voltra_profile::scope("particles");
            self.advance_particles(delta);
        }
        {
            // After the tick too, and for the matching reason: a source the
            // game just spawned or moved is heard from where it is now.
            let _audio = voltra_profile::scope("audio");
            self.update_audio();
        }
        // Spent, whether or not a game installed a tick to read them. Left to
        // pile up they would grow for the length of the session, and a game
        // that installed its hook later would be handed a backlog.
        self.pending_physics.clear();
        // Every frame, simulating or not: the editor draws and picks against
        // a solid layer's colliders while authoring too, and a layer painted
        // by the tick above must be solid in the step that follows it.
        {
            let _physics = voltra_profile::scope("physics");
            voltra_scene::tilemap::sync_colliders(&mut self.world);
            self.step_physics(delta);
        }
        // The fixed tick spawns too, from inside the steps.
        self.resolve_spawned();
        // Last: a scene change despawns what the ticks and steps were holding,
        // so it waits until they have let go.
        self.apply_scene_requests();
        // After the requests, which may have started a change or a world:
        // what loads over frames gets this frame's budget.
        self.update_loading();
        // After every step: the snapshot is the world as the frame left it,
        // and a remote entity is drawn where the clock says, whatever the
        // physics did to it.
        self.send_replication(delta);
        // What every tick of the frame queued goes out together.
        self.flush_net();
    }
}
