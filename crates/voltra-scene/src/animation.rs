//! Walking a sprite through the frames of its atlas.

use voltra_ecs::World;

use crate::sprite::Sprite;

/// A clock that writes [`Sprite::frame`].
///
/// Deliberately not a state machine. Unity's `Animator` graph and Godot's
/// `AnimationTree` are a subsystem of their own — transitions, conditions,
/// blending — and a game today changes clips by writing this component, which
/// is what Bevy's users did for years with a timer and an index.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SpriteAnimation {
    /// Indices into the sprite's atlas, in the order they play.
    ///
    /// A list rather than a range: a run cycle that returns to frame 1 between
    /// 0 and 2 is the common case, and a range cannot say that. Aseprite's
    /// exporter emits exactly this shape too.
    #[serde(default)]
    pub frames: Vec<u32>,
    /// Frames per second. Zero or less freezes rather than dividing by zero.
    #[serde(default = "default_fps")]
    pub fps: f32,
    /// Whether the last frame wraps back to the first.
    ///
    /// Not looping stops on the last frame and stays there, which is what a
    /// death or a hit animation is.
    #[serde(default = "default_true")]
    pub looping: bool,
    /// Whether the clock runs at all. An author toggles this to see the sprite
    /// move in the viewport.
    #[serde(default = "default_true")]
    pub playing: bool,
    /// Seconds into the current cycle.
    ///
    /// Never serialised: where a clip happens to be at save time is not
    /// authoring data, and a scene that reopened mid-blink would be a diff
    /// that changes on every save.
    #[serde(skip)]
    elapsed: f32,
}

fn default_fps() -> f32 {
    SpriteAnimation::DEFAULT_FPS
}

fn default_true() -> bool {
    true
}

impl Default for SpriteAnimation {
    fn default() -> Self {
        Self {
            frames: Vec::new(),
            fps: Self::DEFAULT_FPS,
            looping: true,
            playing: true,
            elapsed: 0.0,
        }
    }
}

impl SpriteAnimation {
    /// The rate a new clip starts at.
    ///
    /// Twelve: animation's traditional "on twos" against 24 fps film, and what
    /// Aseprite's default 100 ms frame comes to within a rounding.
    pub const DEFAULT_FPS: f32 = 12.0;

    /// A looping clip over `frames` at `fps`.
    pub fn new(frames: Vec<u32>, fps: f32) -> Self {
        Self {
            frames,
            fps,
            ..Default::default()
        }
    }

    pub fn with_looping(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }

    /// Seconds into the current cycle. For the editor's read-out.
    pub fn elapsed(&self) -> f32 {
        self.elapsed
    }

    /// Restarts the clip from its first frame.
    pub fn restart(&mut self) {
        self.elapsed = 0.0;
        self.playing = true;
    }

    /// Runs the clock for `delta` seconds and returns the frame to draw, or
    /// `None` when nothing changes.
    ///
    /// Freezes — rather than failing — on an empty list, a rate that is zero,
    /// negative or not finite, and while `playing` is false. Each of those is
    /// a clip an author is part-way through writing, and a scene that panics
    /// while being edited is worse than one that holds still.
    pub fn advance(&mut self, delta: f32) -> Option<u32> {
        if !self.playing || self.frames.is_empty() || !self.fps.is_finite() || self.fps <= 0.0 {
            return None;
        }

        self.elapsed += delta.max(0.0);
        let step = (self.elapsed * self.fps) as usize;

        if step < self.frames.len() {
            return self.frames.get(step).copied();
        }

        if !self.looping {
            // Stops on the last frame and stays there. `playing` goes false so
            // the clock stops accumulating a number nothing reads, and so an
            // editor can see the clip is over.
            self.playing = false;
            self.elapsed = self.frames.len() as f32 / self.fps;
            return self.frames.last().copied();
        }

        // Wrapped on the elapsed time rather than by stepping the index, so a
        // frame that owes two seconds lands on the phase it would have reached
        // had it been drawn all along — and `elapsed` stays bounded however
        // long the clip runs.
        let cycle = self.frames.len() as f32 / self.fps;
        self.elapsed %= cycle;
        let step = (self.elapsed * self.fps) as usize;
        // `as usize` truncates, and a float a hair under the cycle can still
        // reach the length. Clamping beats indexing past the end.
        self.frames.get(step.min(self.frames.len() - 1)).copied()
    }
}

/// Advances every playing animation by `delta` and writes [`Sprite::frame`].
///
/// Per *frame*, not per physics step: what is drawn is not what is simulated,
/// and an animation on the fixed clock would stutter on any frame owing two
/// steps. An entity with an animation and no sprite is skipped — the clip has
/// nothing to write to, and spawning the two components in either order must
/// not matter.
pub fn advance(world: &mut World, delta: f32) {
    let frames: Vec<_> = world
        .query_mut::<SpriteAnimation>()
        .filter_map(|(entity, animation)| animation.advance(delta).map(|frame| (entity, frame)))
        .collect();

    for (entity, frame) in frames {
        if let Some(sprite) = world.get_mut::<Sprite>(entity) {
            sprite.frame = frame;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip() -> SpriteAnimation {
        SpriteAnimation::new(vec![0, 1, 2, 3], 4.0)
    }

    #[test]
    fn one_frame_passes_every_reciprocal_of_the_rate() {
        let mut animation = clip();

        assert_eq!(animation.advance(0.1), Some(0), "still inside the first");
        assert_eq!(animation.advance(0.2), Some(1), "0.3s at 4 fps");
        assert_eq!(animation.advance(0.25), Some(2));
    }

    #[test]
    fn looping_wraps_back_to_the_first_frame() {
        let mut animation = clip();

        animation.advance(1.1);

        assert_eq!(animation.advance(0.0), Some(0), "one whole second wrapped");
    }

    #[test]
    fn a_long_delta_keeps_the_phase_it_would_have_had() {
        // A frame that owes two seconds must not land where per-frame stepping
        // would have left it: a stall must not shift the cycle.
        let mut animation = clip();

        let frame = animation.advance(2.25);

        assert_eq!(frame, Some(1), "2.25s of a 1s cycle is a quarter in");
    }

    #[test]
    fn not_looping_stops_on_the_last_frame_and_stays() {
        let mut animation = clip().with_looping(false);

        assert_eq!(animation.advance(5.0), Some(3));
        assert!(!animation.playing, "and says so");
        assert_eq!(animation.advance(5.0), None, "nothing further to write");
    }

    #[test]
    fn a_rate_of_zero_or_less_freezes_rather_than_dividing() {
        let mut zero = SpriteAnimation::new(vec![0, 1], 0.0);
        let mut negative = SpriteAnimation::new(vec![0, 1], -8.0);
        let mut nonsense = SpriteAnimation::new(vec![0, 1], f32::NAN);

        assert_eq!(zero.advance(10.0), None);
        assert_eq!(negative.advance(10.0), None);
        assert_eq!(nonsense.advance(10.0), None);
    }

    #[test]
    fn an_empty_clip_or_a_paused_one_changes_nothing() {
        let mut empty = SpriteAnimation::new(Vec::new(), 12.0);
        let mut paused = clip();
        paused.playing = false;

        assert_eq!(empty.advance(1.0), None);
        assert_eq!(paused.advance(1.0), None);
    }

    #[test]
    fn restarting_plays_from_the_first_frame_again() {
        let mut animation = clip().with_looping(false);
        animation.advance(5.0);

        animation.restart();

        assert_eq!(animation.advance(0.0), Some(0));
    }

    #[test]
    fn advancing_a_world_writes_the_sprites_frame() {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Sprite::default());
        world.insert(entity, clip());

        advance(&mut world, 0.3);

        assert_eq!(world.get::<Sprite>(entity).expect("the sprite").frame, 1);
    }

    #[test]
    fn an_animation_without_a_sprite_is_skipped() {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, clip());

        advance(&mut world, 0.3);

        assert!(world.get::<Sprite>(entity).is_none());
    }

    #[test]
    fn where_a_clip_had_reached_is_not_saved() {
        let mut animation = clip();
        animation.advance(0.6);

        let text = ron::to_string(&animation).expect("serialize");
        let back: SpriteAnimation = ron::from_str(&text).expect("deserialize");

        assert!(!text.contains("elapsed"), "phase leaked into RON: {text}");
        assert_eq!(back.elapsed(), 0.0);
        assert_eq!(back.frames, animation.frames);
    }

    #[test]
    fn a_clip_written_as_frames_alone_loops_at_the_default_rate() {
        let animation: SpriteAnimation = ron::from_str("(frames: [0, 1])").expect("deserialize");

        assert_eq!(animation.fps, SpriteAnimation::DEFAULT_FPS);
        assert!(animation.looping && animation.playing);
    }
}
