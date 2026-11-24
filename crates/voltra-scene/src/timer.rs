//! Timers: a moment some seconds from now, once or over and over.

use voltra_ecs::{Entity, World};

/// Names a timer, to ask whether it fired and to cancel it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimerId(u64);

#[derive(Debug, Clone, PartialEq)]
struct Timer {
    id: TimerId,
    /// The entity it belongs to: it goes when the entity does.
    owner: Option<Entity>,
    /// Seconds until it next fires.
    left: f32,
    /// `Some(period)` to fire again every `period` seconds.
    every: Option<f32>,
}

/// Every pending timer, and which fired this frame.
///
/// Godot's `get_tree().create_timer()` and `Timer` node, Unity's `Invoke`
/// and `InvokeRepeating`, asked rather than called back: the game checks
/// [`fired`](Self::fired) in its tick, as it checks a button's click. A
/// callback would be the game's code kept by the engine, and a game
/// library reloaded mid-play must leave none of its code behind.
///
/// Stepped once a frame on the frame's delta while the game simulates,
/// before the game's tick. A timer given an owner is dropped when its
/// owner is despawned; a repeating timer whose period passes more than
/// once in one frame fires that many times, counted by
/// [`times`](Self::times).
#[derive(Debug, Clone, Default)]
pub struct Timers {
    pending: Vec<Timer>,
    next: u64,
    fired: Vec<(TimerId, u32)>,
}

/// The shortest period a repeating timer keeps: one shorter would fire
/// without bound in a long frame.
pub const MIN_PERIOD: f32 = 1e-3;

impl Timers {
    /// Fires once, `seconds` from now. With an `owner`, not if the owner is
    /// despawned first.
    pub fn after(&mut self, seconds: f32, owner: Option<Entity>) -> TimerId {
        self.push(seconds, None, owner)
    }

    /// Fires every `seconds`, the first time `seconds` from now, until
    /// cancelled or its owner is despawned.
    pub fn every(&mut self, seconds: f32, owner: Option<Entity>) -> TimerId {
        let period = if seconds.is_finite() {
            seconds.max(MIN_PERIOD)
        } else {
            MIN_PERIOD
        };
        self.push(period, Some(period), owner)
    }

    fn push(&mut self, seconds: f32, every: Option<f32>, owner: Option<Entity>) -> TimerId {
        self.next += 1;
        let id = TimerId(self.next);
        self.pending.push(Timer {
            id,
            owner,
            left: if seconds.is_finite() {
                seconds.max(0.0)
            } else {
                0.0
            },
            every,
        });
        id
    }

    /// Stops `timer` if it is still pending.
    pub fn cancel(&mut self, timer: TimerId) {
        self.pending.retain(|t| t.id != timer);
    }

    /// Whether `timer` fired in the last step.
    pub fn fired(&self, timer: TimerId) -> bool {
        self.times(timer) > 0
    }

    /// How many times `timer` fired in the last step: more than once only
    /// for a repeating timer and a frame longer than its period.
    pub fn times(&self, timer: TimerId) -> u32 {
        self.fired
            .iter()
            .find(|(id, _)| *id == timer)
            .map_or(0, |(_, n)| *n)
    }

    /// Whether `timer` is still to fire.
    pub fn is_pending(&self, timer: TimerId) -> bool {
        self.pending.iter().any(|t| t.id == timer)
    }

    /// Seconds until `timer` next fires, if it is pending.
    pub fn left(&self, timer: TimerId) -> Option<f32> {
        self.pending.iter().find(|t| t.id == timer).map(|t| t.left)
    }

    /// How many timers are pending.
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// Whether no timer is pending.
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Advances every timer by `delta` seconds.
    pub fn step(&mut self, world: &World, delta: f32) {
        self.fired.clear();
        let delta = if delta.is_finite() {
            delta.max(0.0)
        } else {
            0.0
        };
        self.pending
            .retain(|t| t.owner.is_none_or(|owner| world.is_alive(owner)));
        let mut fired = Vec::new();
        self.pending.retain_mut(|timer| {
            timer.left -= delta;
            if timer.left > 0.0 {
                return true;
            }
            match timer.every {
                Some(period) => {
                    let mut times = 0;
                    while timer.left <= 0.0 {
                        timer.left += period;
                        times += 1;
                    }
                    fired.push((timer.id, times));
                    true
                }
                None => {
                    fired.push((timer.id, 1));
                    false
                }
            }
        });
        self.fired = fired;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_timer_fires_once_when_its_time_comes() {
        let world = World::new();
        let mut timers = Timers::default();
        let id = timers.after(1.0, None);
        timers.step(&world, 0.6);
        assert!(!timers.fired(id));
        assert!((timers.left(id).expect("pending") - 0.4).abs() < 1e-6);
        timers.step(&world, 0.6);
        assert!(timers.fired(id) && !timers.is_pending(id));
        timers.step(&world, 5.0);
        assert!(!timers.fired(id));
    }

    #[test]
    fn a_repeating_timer_fires_each_period_and_counts_a_long_frame() {
        let world = World::new();
        let mut timers = Timers::default();
        let id = timers.every(0.5, None);
        timers.step(&world, 0.5);
        assert_eq!(timers.times(id), 1);
        timers.step(&world, 0.2);
        assert_eq!(timers.times(id), 0);
        timers.step(&world, 1.4);
        assert_eq!(timers.times(id), 3, "0.7, 1.2 and 1.7 seconds in, then 2.1");
        assert!(timers.is_pending(id));
        timers.cancel(id);
        timers.step(&world, 10.0);
        assert!(!timers.fired(id));
    }

    #[test]
    fn a_timer_goes_with_its_owner() {
        let mut world = World::new();
        let owner = world.spawn();
        let mut timers = Timers::default();
        let owned = timers.after(1.0, Some(owner));
        let free = timers.after(1.0, None);
        world.despawn(owner);
        timers.step(&world, 2.0);
        assert!(!timers.fired(owned) && !timers.is_pending(owned));
        assert!(timers.fired(free));
    }

    #[test]
    fn a_zero_or_bad_period_does_not_spin() {
        let world = World::new();
        let mut timers = Timers::default();
        let id = timers.every(0.0, None);
        timers.step(&world, 0.01);
        let times = timers.times(id);
        assert!(
            (9..=10).contains(&times),
            "a millisecond apart, bounded: {times}"
        );
        let nan = timers.after(f32::NAN, None);
        timers.step(&world, 0.0);
        assert!(timers.fired(nan));
    }
}
