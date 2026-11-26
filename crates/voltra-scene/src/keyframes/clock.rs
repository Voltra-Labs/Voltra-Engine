//! A player's clock: how far a frame moves it, looping or held on the end.

use super::player::AnimationPlayer;

impl AnimationPlayer {
    /// Runs the clock `delta` seconds on a clip `duration` long. Answers
    /// whether to sample it — while playing, and on the frame it ends.
    ///
    /// A looping clip wraps on the elapsed time, so a stalled frame lands on
    /// the phase it would have reached; a clip of no length loops on its
    /// one pose. A speed that is not finite holds still.
    pub(super) fn step(&mut self, duration: f32, looping: bool, delta: f32) -> bool {
        run_counted(
            &mut self.time,
            &mut self.played,
            &mut self.playing,
            self.speed,
            duration,
            looping,
            delta,
        )
    }
}

/// [`run_clock`], adding to `played` what the clock moved — before a wrap
/// or a clamp took it back, so every loop counts: a player's clock, and a
/// set of synced clips' in lengths.
pub(super) fn run_counted(
    time: &mut f32,
    played: &mut f32,
    playing: &mut bool,
    speed: f32,
    duration: f32,
    looping: bool,
    delta: f32,
) -> bool {
    let before = *time;
    let sampled = run_clock(time, playing, speed, duration, looping, delta);
    if sampled {
        let moved = (delta.max(0.0) * speed).abs();
        *played += if looping {
            moved
        } else {
            (*time - before).abs()
        };
    }
    sampled
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
}
