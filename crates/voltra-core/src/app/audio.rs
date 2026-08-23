//! Playing the scene's own sources, and moving them as they move.
//!
//! The counterpart to [`framing`](super::framing): that one asks which camera
//! the scene is seen through, this one asks where it is heard from and keeps
//! every sounding source in the right ear. Both are the platform layer doing
//! for a shipped game what the editor would otherwise have to do for it.

use std::collections::HashMap;

use voltra_assets::Clips;
use voltra_audio::{spatial, Audio, VoiceId};
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::{audio_listener, AudioSource};

use crate::app::App;

/// Which of the scene's sources are sounding, and where they are heard from.
#[derive(Debug, Default)]
pub(super) struct SceneAudio {
    /// The voice each started source is playing on.
    ///
    /// Also the record of what has already been started: a `play_on_spawn`
    /// source in here is not started again, which is what makes "on spawn"
    /// mean once rather than every frame. Cleared when the world stops, so an
    /// editor's Play starts the scene's sounds afresh.
    playing: HashMap<Entity, VoiceId>,
    /// Whether the missing-listener line has already been logged.
    ///
    /// Latched exactly as [`SceneFraming`](super::framing::SceneFraming)
    /// latches its own: this runs sixty times a second and the sentence does
    /// not change. Cleared as soon as a listener answers.
    warned: bool,
}

impl SceneAudio {
    /// Starts what should be sounding and moves what already is.
    pub(super) fn update(&mut self, world: &World, clips: Option<&Clips>, audio: &mut Audio) {
        let listener = audio_listener::position(world);

        match listener {
            Some(_) => self.warned = false,
            None if !self.warned => {
                log::warn!("no active audio listener in the scene: playing every source flat");
                self.warned = true;
            }
            None => {}
        }

        self.start(world, clips, audio, listener);
        self.follow(world, audio, listener);
    }

    /// Ends everything the scene started and forgets it.
    ///
    /// What an editor's Stop wants, and what a world that is no longer the
    /// same world wants. Sounds a game's tick started go too: they belong to
    /// the run that is ending, and nothing else knows their ids.
    pub(super) fn silence(&mut self, audio: &mut Audio) {
        if self.playing.is_empty() {
            return;
        }
        audio.stop_all();
        self.playing.clear();
    }

    /// How many of the scene's sources have been started.
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.playing.len()
    }

    /// Starts every `play_on_spawn` source that is not already sounding.
    ///
    /// A source whose clip has not resolved is left alone rather than marked
    /// started, so naming a file in the inspector starts it on the next frame
    /// instead of never.
    fn start(
        &mut self,
        world: &World,
        clips: Option<&Clips>,
        audio: &mut Audio,
        listener: Option<Vec2>,
    ) {
        let Some(clips) = clips else {
            return;
        };

        // Collected first: the loop below needs the world again to ask where
        // each source is, and a query holds it borrowed.
        let pending: Vec<Entity> = world
            .query::<AudioSource>()
            .filter(|(entity, source)| source.play_on_spawn && !self.playing.contains_key(entity))
            .map(|(entity, _)| entity)
            .collect();

        for entity in pending {
            let Some(source) = world.get::<AudioSource>(entity) else {
                continue;
            };
            let Some(clip) = source.clip_handle.and_then(|handle| clips.try_get(handle)) else {
                continue;
            };

            let (volume, pan) = mix_for(source, position_of(world, entity), listener);
            let mut params = source.params();
            params.volume = volume;
            params.pan = pan;

            let voice = audio.play(clip, params);
            self.playing.insert(entity, voice);
        }
    }

    /// Moves every sounding source to where its entity now is.
    ///
    /// A voice whose source is gone — the entity despawned, or the component
    /// taken off — is stopped here. Nothing else would ever stop a looping
    /// one, and a despawned entity's ambience playing on is the bug this
    /// exists to prevent.
    fn follow(&mut self, world: &World, audio: &mut Audio, listener: Option<Vec2>) {
        let mut gone = Vec::new();

        for (&entity, &voice) in &self.playing {
            let Some(source) = world.get::<AudioSource>(entity) else {
                gone.push((entity, voice));
                continue;
            };

            let (volume, pan) = mix_for(source, position_of(world, entity), listener);
            audio.set_gain(voice, volume, pan);
        }

        for (entity, voice) in gone {
            audio.stop(voice);
            self.playing.remove(&entity);
        }
    }
}

/// Where `entity` is in world space, composed through its parents.
fn position_of(world: &World, entity: Entity) -> Vec2 {
    voltra_scene::hierarchy::world_matrix(world, entity).transform_point2(Vec2::ZERO)
}

/// The gain and pan a source should be heard at from `listener`.
///
/// With no listener every source plays flat — its own volume, centred —
/// rather than falling silent. A scene where every sound vanished because
/// nobody added a component would be a worse answer than one that plays them
/// all, and it is the same call the missing-camera path already makes.
fn mix_for(source: &AudioSource, at: Vec2, listener: Option<Vec2>) -> (f32, f32) {
    let Some(listener) = listener else {
        return (source.volume, 0.0);
    };

    let offset = at - listener;
    let volume = source.volume * spatial::attenuation(offset.length(), source.range);
    (volume, spatial::pan(offset.x, source.range))
}

impl App {
    /// Runs the scene's audio for one frame.
    ///
    /// Gated on the simulation switch, like the game's tick and the sprite
    /// clocks, and for a sharper version of the same reason: an editor that
    /// played every ambience in the scene while it was being authored would be
    /// unusable in a way a moving sprite is not. Unity is stricter still — an
    /// `AudioSource` previewed in edit mode is a deliberate button — and Stop
    /// silencing everything is exactly this gate's other half.
    pub(super) fn update_audio(&mut self) {
        if self.simulation.is_running() {
            let clips = self.clips.as_ref();
            self.scene_audio.update(&self.world, clips, &mut self.audio);
        } else {
            self.scene_audio.silence(&mut self.audio);
        }

        // Whether or not anything is playing: the clips finished voices handed
        // back are freed here, on the thread that can afford to.
        self.audio.end_frame();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_assets::AssetPath;
    use voltra_scene::{AudioListener, Transform};
    use voltra_testkit::scratch_root;

    /// A store holding one clip under `coin.wav`.
    ///
    /// The file is not there, so the clip is silent — which is all these tests
    /// need. What is being tested is which voices exist and where they are,
    /// and that is decided before a sample is read.
    fn clips_with_a_coin() -> (Clips, voltra_assets::Handle<voltra_audio::Clip>) {
        let mut clips = Clips::new(scratch_root());
        let handle = clips.load(&AssetPath::new("coin.wav").expect("a valid path"));
        (clips, handle)
    }

    fn spawn_source(world: &mut World, source: AudioSource, at: Vec2) -> Entity {
        let entity = world.spawn();
        world.insert(entity, Transform::from_translation(at));
        world.insert(entity, source);
        entity
    }

    fn spawn_listener(world: &mut World, at: Vec2) -> Entity {
        let entity = world.spawn();
        world.insert(entity, Transform::from_translation(at));
        world.insert(entity, AudioListener::default());
        entity
    }

    #[test]
    fn a_play_on_spawn_source_starts_once() {
        let (clips, handle) = clips_with_a_coin();
        let mut world = World::new();
        spawn_source(
            &mut world,
            AudioSource {
                clip_handle: Some(handle),
                play_on_spawn: true,
                ..Default::default()
            },
            Vec2::ZERO,
        );
        let mut scene_audio = SceneAudio::default();
        let mut audio = Audio::silent();

        scene_audio.update(&world, Some(&clips), &mut audio);
        scene_audio.update(&world, Some(&clips), &mut audio);

        assert_eq!(
            scene_audio.len(),
            1,
            "the second frame must not start it again"
        );
    }

    #[test]
    fn a_source_that_does_not_play_on_spawn_is_left_to_the_game() {
        let (clips, handle) = clips_with_a_coin();
        let mut world = World::new();
        spawn_source(
            &mut world,
            AudioSource {
                clip_handle: Some(handle),
                ..Default::default()
            },
            Vec2::ZERO,
        );
        let mut scene_audio = SceneAudio::default();

        scene_audio.update(&world, Some(&clips), &mut Audio::silent());

        assert_eq!(scene_audio.len(), 0);
    }

    #[test]
    fn a_source_with_no_clip_yet_is_started_when_one_arrives() {
        // Naming a file in the inspector has to start it on the next frame,
        // not never — which is what marking it started would have meant.
        let (clips, handle) = clips_with_a_coin();
        let mut world = World::new();
        let entity = spawn_source(
            &mut world,
            AudioSource {
                play_on_spawn: true,
                ..Default::default()
            },
            Vec2::ZERO,
        );
        let mut scene_audio = SceneAudio::default();
        let mut audio = Audio::silent();

        scene_audio.update(&world, Some(&clips), &mut audio);
        assert_eq!(scene_audio.len(), 0);

        world
            .get_mut::<AudioSource>(entity)
            .expect("it is there")
            .clip_handle = Some(handle);
        scene_audio.update(&world, Some(&clips), &mut audio);

        assert_eq!(scene_audio.len(), 1);
    }

    #[test]
    fn a_source_whose_entity_is_gone_is_stopped() {
        // The bug this prevents: a looping ambience outliving the entity that
        // carried it, with nothing left holding its id.
        let (clips, handle) = clips_with_a_coin();
        let mut world = World::new();
        let entity = spawn_source(
            &mut world,
            AudioSource {
                clip_handle: Some(handle),
                play_on_spawn: true,
                looping: true,
                ..Default::default()
            },
            Vec2::ZERO,
        );
        let mut scene_audio = SceneAudio::default();
        let mut audio = Audio::silent();
        scene_audio.update(&world, Some(&clips), &mut audio);
        assert_eq!(scene_audio.len(), 1);

        world.despawn(entity);
        scene_audio.update(&world, Some(&clips), &mut audio);

        assert_eq!(scene_audio.len(), 0);
    }

    #[test]
    fn silencing_forgets_everything_so_play_starts_it_again() {
        // An editor's Stop, then Play.
        let (clips, handle) = clips_with_a_coin();
        let mut world = World::new();
        spawn_source(
            &mut world,
            AudioSource {
                clip_handle: Some(handle),
                play_on_spawn: true,
                ..Default::default()
            },
            Vec2::ZERO,
        );
        let mut scene_audio = SceneAudio::default();
        let mut audio = Audio::silent();
        scene_audio.update(&world, Some(&clips), &mut audio);

        scene_audio.silence(&mut audio);
        assert_eq!(scene_audio.len(), 0);

        scene_audio.update(&world, Some(&clips), &mut audio);
        assert_eq!(scene_audio.len(), 1);
    }

    #[test]
    fn a_source_on_the_listener_is_at_full_volume_and_centred() {
        let mut world = World::new();
        spawn_listener(&mut world, Vec2::new(4.0, 4.0));
        let source = AudioSource {
            volume: 0.5,
            range: 10.0,
            ..Default::default()
        };

        let (volume, pan) = mix_for(
            &source,
            Vec2::new(4.0, 4.0),
            audio_listener::position(&world),
        );

        assert!((volume - 0.5).abs() < 1e-6);
        assert_eq!(pan, 0.0);
    }

    #[test]
    fn a_source_to_the_right_of_the_listener_pans_right_and_fades() {
        let mut world = World::new();
        spawn_listener(&mut world, Vec2::ZERO);
        let source = AudioSource {
            range: 10.0,
            ..Default::default()
        };

        let (volume, pan) = mix_for(
            &source,
            Vec2::new(5.0, 0.0),
            audio_listener::position(&world),
        );

        assert!((pan - 0.5).abs() < 1e-6);
        assert!((volume - 0.25).abs() < 1e-6, "got {volume}");
    }

    #[test]
    fn a_source_beyond_its_range_is_silent() {
        let mut world = World::new();
        spawn_listener(&mut world, Vec2::ZERO);
        let source = AudioSource {
            range: 3.0,
            ..Default::default()
        };

        let (volume, _) = mix_for(
            &source,
            Vec2::new(0.0, 40.0),
            audio_listener::position(&world),
        );

        assert_eq!(volume, 0.0, "and height counts towards the distance");
    }

    #[test]
    fn a_scene_with_no_listener_plays_every_source_flat() {
        // Not silence: a scene nobody added a listener to must still be
        // audible, with the reason in the log.
        let world = World::new();
        let source = AudioSource {
            volume: 0.75,
            range: 1.0,
            ..Default::default()
        };

        let (volume, pan) = mix_for(
            &source,
            Vec2::new(500.0, 0.0),
            audio_listener::position(&world),
        );

        assert!((volume - 0.75).abs() < 1e-6);
        assert_eq!(pan, 0.0);
    }

    #[test]
    fn a_source_with_no_range_is_heard_wherever_the_listener_is() {
        // Music and UI. The reason `range` is a number rather than a flag
        // beside one.
        let mut world = World::new();
        spawn_listener(&mut world, Vec2::new(-300.0, 90.0));
        let source = AudioSource {
            range: 0.0,
            ..Default::default()
        };

        let (volume, pan) = mix_for(&source, Vec2::ZERO, audio_listener::position(&world));

        assert_eq!(volume, 1.0);
        assert_eq!(pan, 0.0);
    }

    #[test]
    fn a_parented_source_is_heard_where_its_parent_put_it() {
        let mut world = World::new();
        let rig = world.spawn();
        world.insert(rig, voltra_scene::SceneId::new());
        world.insert(rig, Transform::from_translation(Vec2::new(6.0, 0.0)));

        let entity = spawn_source(&mut world, AudioSource::default(), Vec2::new(1.0, 0.0));
        world.insert(entity, voltra_scene::SceneId::new());
        voltra_scene::hierarchy::set_parent(&mut world, entity, rig).expect("a plain reparent");

        assert!((position_of(&world, entity) - Vec2::new(7.0, 0.0)).length() < 1e-5);
    }

    #[test]
    fn a_frame_with_no_store_starts_nothing_and_does_not_panic() {
        // Every frame before the window exists.
        let mut world = World::new();
        spawn_source(
            &mut world,
            AudioSource {
                play_on_spawn: true,
                ..Default::default()
            },
            Vec2::ZERO,
        );
        let mut scene_audio = SceneAudio::default();

        scene_audio.update(&world, None, &mut Audio::silent());

        assert_eq!(scene_audio.len(), 0);
    }

    #[test]
    fn an_app_that_is_not_simulating_starts_nothing() {
        let mut app = App::default();
        let entity = app.world.spawn();
        app.world.insert(entity, Transform::default());
        app.world.insert(
            entity,
            AudioSource {
                play_on_spawn: true,
                ..Default::default()
            },
        );
        let (clips, handle) = clips_with_a_coin();
        app.clips = Some(clips);
        app.world
            .get_mut::<AudioSource>(entity)
            .expect("it is there")
            .clip_handle = Some(handle);

        app.update_audio();

        assert_eq!(app.scene_audio.len(), 0, "the scene is being authored");
    }

    #[test]
    fn a_live_app_starts_the_scene_s_sources() {
        let mut app = App::default().with_simulation();
        let entity = app.world.spawn();
        app.world.insert(entity, Transform::default());
        let (clips, handle) = clips_with_a_coin();
        app.clips = Some(clips);
        app.world.insert(
            entity,
            AudioSource {
                clip_handle: Some(handle),
                play_on_spawn: true,
                ..Default::default()
            },
        );

        app.update_audio();

        assert_eq!(app.scene_audio.len(), 1);
    }

    #[test]
    fn stopping_the_simulation_silences_the_scene() {
        let mut app = App::default().with_simulation();
        let entity = app.world.spawn();
        app.world.insert(entity, Transform::default());
        let (clips, handle) = clips_with_a_coin();
        app.clips = Some(clips);
        app.world.insert(
            entity,
            AudioSource {
                clip_handle: Some(handle),
                play_on_spawn: true,
                looping: true,
                ..Default::default()
            },
        );
        app.update_audio();
        assert_eq!(app.scene_audio.len(), 1);

        app.set_simulating(false);
        app.update_audio();

        assert_eq!(app.scene_audio.len(), 0);
    }
}
