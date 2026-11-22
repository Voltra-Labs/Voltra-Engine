//! The component that plays a clip on its entity and the ones named under it.

use serde::{Deserialize, Serialize};
use voltra_assets::AssetPath;

/// Plays [`clip`](Self::clip) on this entity, and on the children its
/// tracks name.
///
/// Godot's `AnimationPlayer` shape — one clip at a time, played from the
/// root of what it animates — without its library of named clips: a game
/// switches by writing [`play`](Self::play), as with `SpriteAnimation`.
/// Blending and state machines are a subsystem of their own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnimationPlayer {
    /// The `.anim.ron` it plays; `None` plays nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip: Option<AssetPath>,
    /// Whether the clock runs and the clip is applied. A clip that does not
    /// loop turns this off at its end, having applied its last frame.
    pub playing: bool,
    /// How fast it plays; negative plays it backwards from where it is.
    pub speed: f32,
    /// Seconds into the clip. Never saved: where a clip was at save time is
    /// not authoring data.
    #[serde(skip)]
    time: f32,
    /// The clip whose missing targets were last logged, so a track naming a
    /// child that is not there warns once rather than every frame.
    #[serde(skip)]
    pub(super) reported: Option<AssetPath>,
}

impl Default for AnimationPlayer {
    fn default() -> Self {
        Self {
            clip: None,
            playing: true,
            speed: 1.0,
            time: 0.0,
            reported: None,
        }
    }
}

impl AnimationPlayer {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "AnimationPlayer";

    /// A player of `clip`, playing.
    pub fn new(clip: AssetPath) -> Self {
        Self {
            clip: Some(clip),
            ..Self::default()
        }
    }

    /// Seconds into the clip.
    pub fn time(&self) -> f32 {
        self.time
    }

    /// Moves the clock to `time` without playing — an editor's scrub. The
    /// next sample clamps or wraps it to the clip.
    pub fn seek(&mut self, time: f32) {
        self.time = if time.is_finite() { time } else { 0.0 };
    }

    /// Plays `clip` from its start. The same clip again restarts it.
    pub fn play(&mut self, clip: AssetPath) {
        self.clip = Some(clip);
        self.restart();
    }

    /// Plays from the start again.
    pub fn restart(&mut self) {
        self.time = 0.0;
        self.playing = true;
    }

    /// Runs the clock `delta` seconds on a clip `duration` long. Answers
    /// whether to sample it — while playing, and on the frame it ends.
    ///
    /// A looping clip wraps on the elapsed time, so a stalled frame lands on
    /// the phase it would have reached; a clip of no length loops on its
    /// one pose. A speed that is not finite holds still.
    pub(super) fn step(&mut self, duration: f32, looping: bool, delta: f32) -> bool {
        if !self.playing || !self.speed.is_finite() {
            return false;
        }
        self.time += delta.max(0.0) * self.speed;
        if duration <= 0.0 {
            self.time = 0.0;
            self.playing = looping;
            return true;
        }
        if looping {
            self.time = self.time.rem_euclid(duration);
            return true;
        }
        let ended =
            (self.speed > 0.0 && self.time >= duration) || (self.speed < 0.0 && self.time <= 0.0);
        self.time = self.time.clamp(0.0, duration);
        if ended {
            self.playing = false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clip_that_does_not_loop_stops_on_its_end_and_one_that_does_wraps() {
        let mut player = AnimationPlayer::default();
        assert!(player.step(1.0, false, 0.4));
        assert!(player.step(1.0, false, 0.9), "the end is applied");
        assert_eq!((player.time(), player.playing), (1.0, false));
        assert!(!player.step(1.0, false, 0.1));

        player.restart();
        player.step(1.0, true, 2.25);
        assert!((player.time() - 0.25).abs() < 1e-6);
        player.speed = -1.0;
        player.step(1.0, true, 0.5);
        assert!((player.time() - 0.75).abs() < 1e-6, "backwards wraps too");
        player.step(1.0, false, 5.0);
        assert_eq!((player.time(), player.playing), (0.0, false));
    }

    #[test]
    fn a_default_player_writes_nothing_but_its_defaults() {
        let text = ron::to_string(&AnimationPlayer::default()).expect("writes");
        assert!(!text.contains("time") && !text.contains("clip"), "{text}");
        let back: AnimationPlayer =
            ron::from_str("(clip: Some(Path(\"a.anim.ron\")))").expect("reads");
        assert!(back.playing && back.speed == 1.0);
    }
}
