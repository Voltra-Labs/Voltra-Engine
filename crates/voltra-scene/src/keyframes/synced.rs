//! Several clips on one clock, each at a weight: what a blend tree plays.

use voltra_assets::{AssetPath, Handle};

use super::clip::AnimationClip;
use super::clock;
use super::store::AnimationClips;

/// Clips played together on one clock in normalized time, mixed by weight —
/// a walk and a run blended keep their feet in step, since each is the
/// same fraction of the way through its own cycle.
///
/// The set's length is the weight-averaged length of its clips, so a mix
/// half walk (one second) and half run (half a second) loops every three
/// quarters of a second, each clip sampled at that fraction of its own
/// length. Unity's blend tree sync, Godot's `sync` on a blend space. What
/// sets the weights — a parameter through a tree's thresholds, a game
/// directly — is the caller's; [`weights_mut`](Self::weights_mut) changes
/// them as it plays without moving the clock.
///
/// A clip of no length (a held pose) has no cycle to keep in step: it
/// weighs in the mix, not in the length. The set loops when any of its
/// clips does; a clip that does not, in a set that loops, plays through
/// once and holds its end.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SyncedClips {
    clips: Vec<AssetPath>,
    weights: Vec<f32>,
    /// Through the loop it is in, `0..=1`.
    phase: f32,
    /// Lengths played since it started, through every loop.
    played: f32,
    /// Whether it last ran backwards: where a clip played through, and not
    /// looping, is held — its start rather than its end.
    backwards: bool,
    /// Whether a target it animates was found missing, and said so: once
    /// per set played, as a player warns once per clip.
    pub(super) reported: bool,
}

/// What a set's clips come to, as the store has them.
struct Survey {
    length: f32,
    looping: bool,
    audible: bool,
}

/// One clip of a set to sample this frame: which, when, and how much.
#[derive(Debug, Clone, Copy)]
pub(super) struct Pick {
    pub handle: Handle<AnimationClip>,
    pub time: f32,
    pub weight: f32,
}

impl SyncedClips {
    /// `clips`, each at its weight, from their start.
    pub fn new(clips: impl IntoIterator<Item = (AssetPath, f32)>) -> Self {
        let (clips, weights) = clips.into_iter().unzip();
        Self {
            clips,
            weights,
            ..Self::default()
        }
    }

    /// The clips, in the order they were given.
    pub fn clips(&self) -> &[AssetPath] {
        &self.clips
    }

    /// Each clip's weight, in the order of [`clips`](Self::clips). A weight
    /// of zero, or one that is not a finite number, leaves its clip out.
    pub fn weights(&self) -> &[f32] {
        &self.weights
    }

    /// The weights, to change while it plays: the clock keeps its place.
    pub fn weights_mut(&mut self) -> &mut [f32] {
        &mut self.weights
    }

    /// Swaps in other clips at other weights, keeping the clock where it
    /// is — a reloaded tree that gained or lost a child plays on in step.
    pub fn replace(&mut self, clips: impl IntoIterator<Item = (AssetPath, f32)>) {
        (self.clips, self.weights) = clips.into_iter().unzip();
        self.reported = false;
    }

    /// How far through the loop it is in, `0..=1`.
    pub fn phase(&self) -> f32 {
        self.phase
    }

    /// Lengths played since it started, counting every loop: `2.5` is two
    /// loops and a half.
    pub fn played(&self) -> f32 {
        self.played
    }

    /// Back to the start.
    pub(super) fn rewind(&mut self) {
        self.phase = 0.0;
        self.played = 0.0;
        self.reported = false;
    }

    /// Whether it loops: when any of its clips that reads does.
    pub fn looping(&self, store: &mut AnimationClips) -> bool {
        self.survey(store).looping
    }

    /// Its length, in seconds: the weight-averaged length of the clips that
    /// weigh something and have one; zero when none does.
    pub fn length(&self, store: &mut AnimationClips) -> f32 {
        self.survey(store).length
    }

    /// Its length, whether it loops and whether anything of it would be
    /// written, from one look at each clip.
    fn survey(&self, store: &mut AnimationClips) -> Survey {
        let (mut sum, mut total) = (0.0, 0.0);
        let (mut looping, mut audible) = (false, false);
        for (path, &weight) in self.clips.iter().zip(&self.weights) {
            let clip = read(store, path);
            if clip.is_empty() {
                continue;
            }
            looping |= clip.looping;
            if !weighs(weight) {
                continue;
            }
            audible = true;
            let length = clip.duration();
            if length > 0.0 {
                sum += weight * length;
                total += weight;
            }
        }
        let length = if total > 0.0 && (sum / total).is_finite() {
            sum / total
        } else {
            0.0
        };
        Survey {
            length,
            looping,
            audible,
        }
    }

    /// Runs the clock `delta` seconds at `speed`, on the player's `playing`
    /// rules. Answers whether to sample it.
    ///
    /// A set with nothing that would be written — no clip that reads, at a
    /// weight — neither moves its clock nor ends, as a player of an empty
    /// clip does not.
    pub(super) fn step(
        &mut self,
        store: &mut AnimationClips,
        playing: &mut bool,
        speed: f32,
        delta: f32,
    ) -> bool {
        let Survey {
            length,
            looping,
            audible,
        } = self.survey(store);
        if !audible {
            return false;
        }
        if speed.is_finite() && speed != 0.0 {
            self.backwards = speed < 0.0;
        }
        if length <= 0.0 {
            // Held poses only: no cycle to move through, so the phase stays
            // where it was (a weight that brings a clip with a length back
            // finds it there); a set that does not loop has ended.
            if !*playing || !speed.is_finite() {
                return false;
            }
            *playing = looping;
            return true;
        }
        // In lengths: a second of a set three quarters of a second long
        // moves it one and a third.
        clock::run_counted(
            &mut self.phase,
            &mut self.played,
            playing,
            speed / length,
            1.0,
            looping,
            delta,
        )
    }

    /// The clips to sample now, at what time and weight, onto `picks`:
    /// each at the set's phase of its own length — or held once played
    /// through, for one that does not loop: on its end, or on its start
    /// when the set plays backwards.
    pub(super) fn pick(&self, store: &mut AnimationClips, picks: &mut Vec<Pick>) {
        for (path, &weight) in self.clips.iter().zip(&self.weights) {
            if !weighs(weight) {
                continue;
            }
            let handle = store.load(path);
            let clip = store.get(handle);
            if clip.is_empty() {
                continue;
            }
            let length = clip.duration();
            let held = !clip.looping && self.played >= 1.0;
            let time = match (held, self.backwards) {
                (true, false) => length,
                (true, true) => 0.0,
                (false, _) => self.phase * length,
            };
            picks.push(Pick {
                handle,
                time,
                weight,
            });
        }
    }

    /// The clips, by name, for a warning.
    pub(super) fn describe(&self) -> String {
        let names: Vec<&str> = self.clips.iter().map(AssetPath::as_str).collect();
        format!("the blend of {}", names.join(", "))
    }
}

/// The clip at `path`, read the first time.
fn read<'s>(store: &'s mut AnimationClips, path: &AssetPath) -> &'s AnimationClip {
    let handle = store.load(path);
    store.get(handle)
}

/// Whether a weight counts: a finite amount above zero.
fn weighs(weight: f32) -> bool {
    weight.is_finite() && weight > 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(files: &[(&str, &str)]) -> AnimationClips {
        let root = voltra_testkit::scratch_root();
        for (name, text) in files {
            std::fs::write(root.join(name), text).expect("writes");
        }
        AnimationClips::new(root)
    }

    fn path(name: &str) -> AssetPath {
        AssetPath::new(name).expect("valid")
    }

    const WALK: &str = "(looping: true, length: 1.0, tracks: [(property: Rotation, keys: [(time: 0.0, value: Float(0.0))])])";
    const RUN: &str = "(looping: true, length: 0.5, tracks: [(property: Rotation, keys: [(time: 0.0, value: Float(1.0))])])";
    const POSE: &str = "(tracks: [(property: Rotation, keys: [(time: 0.0, value: Float(2.0))])])";

    #[test]
    fn the_length_is_weight_averaged_and_each_clip_keeps_its_phase() {
        let mut clips = store(&[
            ("walk.anim.ron", WALK),
            ("run.anim.ron", RUN),
            ("pose.anim.ron", POSE),
        ]);
        let mut set = SyncedClips::new([
            (path("walk.anim.ron"), 0.5),
            (path("run.anim.ron"), 0.5),
            (path("pose.anim.ron"), 1.0),
        ]);
        assert!(
            (set.length(&mut clips) - 0.75).abs() < 1e-6,
            "a pose has none"
        );
        let mut playing = true;
        assert!(set.step(&mut clips, &mut playing, 1.0, 0.375));
        assert!((set.phase() - 0.5).abs() < 1e-6);
        let mut picks = Vec::new();
        set.pick(&mut clips, &mut picks);
        let times: Vec<f32> = picks.iter().map(|p| p.time).collect();
        assert_eq!(times, [0.5, 0.25, 0.0], "each half way through its own");

        set.step(&mut clips, &mut playing, 1.0, 0.75);
        assert!((set.phase() - 0.5).abs() < 1e-5, "{}", set.phase());
        assert!((set.played() - 1.5).abs() < 1e-5);
        assert!(playing, "it loops");
    }

    #[test]
    fn weights_change_without_moving_the_clock_and_zero_is_left_out() {
        let mut clips = store(&[("walk.anim.ron", WALK), ("run.anim.ron", RUN)]);
        let mut set = SyncedClips::new([(path("walk.anim.ron"), 1.0), (path("run.anim.ron"), 0.0)]);
        let mut playing = true;
        set.step(&mut clips, &mut playing, 1.0, 0.25);
        assert!((set.phase() - 0.25).abs() < 1e-6, "all walk: a second long");
        set.weights_mut().copy_from_slice(&[0.0, 1.0]);
        assert_eq!(set.phase(), 0.25, "kept");
        assert!((set.length(&mut clips) - 0.5).abs() < 1e-6);
        let mut picks = Vec::new();
        set.pick(&mut clips, &mut picks);
        assert_eq!(picks.len(), 1);
        assert!((picks[0].time - 0.125).abs() < 1e-6);

        set.weights_mut().copy_from_slice(&[f32::NAN, 0.0]);
        assert!(
            !set.step(&mut clips, &mut playing, 1.0, 0.5),
            "nothing weighs"
        );
        assert_eq!(set.phase(), 0.25, "nor moves");
        assert_eq!(set.length(&mut clips), 0.0);
    }

    #[test]
    fn a_set_of_clips_played_once_ends_and_one_held_in_a_loop_holds_its_end() {
        let once = "(length: 1.0, tracks: [(property: Rotation, keys: [(time: 0.0, value: Float(0.0)), (time: 1.0, value: Float(1.0))])])";
        let mut clips = store(&[("once.anim.ron", once), ("walk.anim.ron", WALK)]);
        let mut set = SyncedClips::new([(path("once.anim.ron"), 1.0)]);
        let mut playing = true;
        set.step(&mut clips, &mut playing, 1.0, 1.5);
        assert!(!playing && set.phase() == 1.0, "ended on its end");

        let mut set =
            SyncedClips::new([(path("once.anim.ron"), 0.5), (path("walk.anim.ron"), 0.5)]);
        let mut playing = true;
        set.step(&mut clips, &mut playing, 1.0, 1.25);
        assert!(playing);
        let mut picks = Vec::new();
        set.pick(&mut clips, &mut picks);
        assert_eq!(picks[0].time, 1.0, "the clip played once holds its end");
        assert!((picks[1].time - 0.25).abs() < 1e-5);

        set.step(&mut clips, &mut playing, -1.0, 0.1);
        picks.clear();
        set.pick(&mut clips, &mut picks);
        assert_eq!(picks[0].time, 0.0, "backwards: held on its start");
    }

    #[test]
    fn a_set_of_held_poses_keeps_its_phase() {
        let mut clips = store(&[("walk.anim.ron", WALK), ("pose.anim.ron", POSE)]);
        let mut set =
            SyncedClips::new([(path("walk.anim.ron"), 1.0), (path("pose.anim.ron"), 0.0)]);
        let mut playing = true;
        set.step(&mut clips, &mut playing, 1.0, 0.25);
        set.weights_mut().copy_from_slice(&[0.0, 1.0]);
        assert!(
            set.step(&mut clips, &mut playing, 1.0, 0.5),
            "the pose is sampled"
        );
        assert_eq!(set.phase(), 0.25, "no length to move through");
        assert!(playing, "the walk, at no weight, still loops the set");
        set.weights_mut().copy_from_slice(&[1.0, 0.0]);
        set.step(&mut clips, &mut playing, 1.0, 0.25);
        assert!((set.phase() - 0.5).abs() < 1e-6, "on from where it was");
    }
}
