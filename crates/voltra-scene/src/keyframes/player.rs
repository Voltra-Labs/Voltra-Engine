//! The component that plays a clip on its entity and the ones named under it.

use serde::{Deserialize, Serialize};
use voltra_assets::AssetPath;

use super::fade::{Fade, Outgoing, Playing};

/// Plays [`clip`](Self::clip) on this entity, and on the children its
/// tracks name.
///
/// Godot's `AnimationPlayer` shape — one clip at a time, played from the
/// root of what it animates — without its library of named clips: a game
/// switches by writing [`play`](Self::play), as with `SpriteAnimation`, or
/// fades into the next with [`cross_fade`](Self::cross_fade). Choosing the
/// clip is an [`Animator`](crate::Animator)'s.
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
    /// Seconds of the clip played since it started, through every loop —
    /// what an exit time above one counts. Never saved.
    #[serde(skip)]
    played: f32,
    /// The fade into [`clip`](Self::clip) from what played before, while
    /// one runs. Never saved: a scene loads on its clip, not mid-fade.
    #[serde(skip)]
    pub(super) fade: Option<Fade>,
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
            played: 0.0,
            fade: None,
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

    /// Seconds of the clip played since it started, counting every loop
    /// and either direction: `2.5` lengths into a looping clip is two loops
    /// and a half.
    pub fn played(&self) -> f32 {
        self.played
    }

    /// The weight of [`clip`](Self::clip) while a fade into it runs, `0..1`;
    /// `None` when it plays alone.
    pub fn fade_weight(&self) -> Option<f32> {
        self.fade.as_ref().map(Fade::weight)
    }

    /// Moves the clock to `time` without playing — an editor's scrub. The
    /// next sample clamps or wraps it to the clip.
    pub fn seek(&mut self, time: f32) {
        self.time = if time.is_finite() { time } else { 0.0 };
        self.played = self.time.abs();
    }

    /// Plays `clip` from its start, cutting to it: whatever played before
    /// stops being written at once. The same clip again restarts it.
    pub fn play(&mut self, clip: AssetPath) {
        self.clip = Some(clip);
        self.restart();
    }

    /// Plays `clip` from its start, fading into it over `duration` seconds:
    /// until then each property is written once, with the mix of what
    /// played before — which keeps playing from where it was — and `clip`,
    /// `clip`'s weight going from 0 to 1 linearly. Unity's `CrossFade`,
    /// Godot's `xfade_time`.
    ///
    /// Called mid-fade, it fades from the mix last written, held still, so
    /// nothing jumps. A `duration` that is not a positive number, or a
    /// player that had no clip, is [`play`](Self::play): a cut.
    pub fn cross_fade(&mut self, clip: AssetPath, duration: f32) {
        let fades = duration.is_finite() && duration > 0.0;
        let from = match (fades, self.fade.take(), self.clip.take()) {
            (false, ..) | (true, None, None) => None,
            // Interrupted after a frame of mixing: what is on screen.
            (
                true,
                Some(Fade {
                    last: Some(mix), ..
                }),
                _,
            ) => Some(Outgoing::Pose(mix)),
            // Interrupted before any: still all the old side.
            (true, Some(fade), _) => Some(fade.from),
            (true, None, Some(previous)) => Some(Outgoing::Clip(Playing {
                clip: previous,
                time: self.time,
                speed: self.speed,
                playing: self.playing,
            })),
        };
        self.play(clip);
        self.fade = from.map(|from| Fade::new(from, duration));
    }

    /// Plays from the start again, cutting any fade short.
    pub fn restart(&mut self) {
        self.time = 0.0;
        self.played = 0.0;
        self.playing = true;
        self.fade = None;
    }

    /// Runs the clock `delta` seconds on a clip `duration` long. Answers
    /// whether to sample it — while playing, and on the frame it ends.
    ///
    /// A looping clip wraps on the elapsed time, so a stalled frame lands on
    /// the phase it would have reached; a clip of no length loops on its
    /// one pose. A speed that is not finite holds still.
    pub(super) fn step(&mut self, duration: f32, looping: bool, delta: f32) -> bool {
        let before = self.time;
        let sampled = run_clock(
            &mut self.time,
            &mut self.playing,
            self.speed,
            duration,
            looping,
            delta,
        );
        if sampled {
            // What the clock moved, before a wrap or a clamp took it back.
            let moved = (delta.max(0.0) * self.speed).abs();
            self.played += if looping {
                moved
            } else {
                (self.time - before).abs()
            };
        }
        sampled
    }
}

/// [`AnimationPlayer::step`]'s clock, on its parts: a fade's outgoing clip
/// runs on the same rules without being a player.
pub(super) fn run_clock(
    time: &mut f32,
    playing: &mut bool,
    speed: f32,
    duration: f32,
    looping: bool,
    delta: f32,
) -> bool {
    if !*playing || !speed.is_finite() {
        return false;
    }
    *time += delta.max(0.0) * speed;
    if duration <= 0.0 {
        *time = 0.0;
        *playing = looping;
        return true;
    }
    if looping {
        *time = time.rem_euclid(duration);
        return true;
    }
    let ended = (speed > 0.0 && *time >= duration) || (speed < 0.0 && *time <= 0.0);
    *time = time.clamp(0.0, duration);
    if ended {
        *playing = false;
    }
    true
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
    fn played_counts_every_loop_and_a_restart_clears_it() {
        let mut player = AnimationPlayer::default();
        player.step(1.0, true, 2.5);
        assert!((player.played() - 2.5).abs() < 1e-6);
        assert!((player.time() - 0.5).abs() < 1e-6);
        player.restart();
        assert_eq!(player.played(), 0.0);
        player.step(1.0, false, 3.0);
        assert_eq!(
            player.played(),
            1.0,
            "a clip that ends plays only its length"
        );
    }

    #[test]
    fn a_cross_fade_of_no_length_or_from_nothing_is_a_cut() {
        let clip = |name: &str| AssetPath::new(name).expect("valid");
        let mut player = AnimationPlayer::default();
        player.cross_fade(clip("a.anim.ron"), 0.3);
        assert_eq!(player.fade_weight(), None, "nothing played before");
        player.cross_fade(clip("b.anim.ron"), 0.3);
        assert_eq!(player.fade_weight(), Some(0.0));
        player.cross_fade(clip("c.anim.ron"), f32::NAN);
        assert_eq!(player.fade_weight(), None);
        player.cross_fade(clip("d.anim.ron"), 0.3);
        player.play(clip("e.anim.ron"));
        assert_eq!(player.fade_weight(), None, "play cuts a fade short");
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
