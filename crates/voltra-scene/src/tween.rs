//! Tweens: a property of an entity moved to a value over time, from code.

use voltra_ecs::{Entity, World};

use crate::keyframes::{Ease, Property, Value};

/// Names a tween, for chaining, cancelling and asking whether it finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TweenId(u64);

/// One tween: where it is going, how, and how far it has got.
#[derive(Debug, Clone, PartialEq)]
struct Tween {
    id: TweenId,
    entity: Entity,
    property: Property,
    /// Read when it starts — when its turn in a chain comes — so it moves
    /// from where the previous one left the property.
    from: Option<Value>,
    to: Value,
    seconds: f32,
    ease: Ease,
    elapsed: f32,
    /// Waiting for this one to finish before it starts.
    after: Option<TweenId>,
}

/// Every running tween, and what finished this frame.
///
/// Godot's `create_tween().tween_property(…)`, Unity's DOTween, over the
/// [`Property`] list animation clips already address and with their
/// [`Ease`]s: one code path writes a property for both. Stepped once a
/// frame on the frame's delta while the game simulates, before the game's
/// tick, so the tick reads this frame's [`finished`](Self::finished).
///
/// A tween dies with its entity — a despawned or replaced one is dropped
/// the next step, and the chain behind it with it — and a second tween on
/// the same property of the same entity takes over from the first, which
/// is cancelled: the newest target wins, as a retargeted move expects.
/// Nothing a game passes is kept but values, so a game reloaded mid-play
/// leaves no code of the old library behind.
#[derive(Debug, Clone, Default)]
pub struct Tweens {
    running: Vec<Tween>,
    next: u64,
    finished: Vec<TweenId>,
}

impl Tweens {
    /// Moves `property` of `entity` to `to` over `seconds`, shaped by
    /// `ease`, starting now. A value of another kind than the property's,
    /// or one that is not a number, is refused — logged, and finished at
    /// once so a chain behind it still runs.
    pub fn tween(
        &mut self,
        entity: Entity,
        property: Property,
        to: Value,
        seconds: f32,
        ease: Ease,
    ) -> TweenId {
        self.push(entity, property, to, seconds, ease, None)
    }

    /// As [`tween`](Self::tween), starting when `previous` finishes — or at
    /// once if it already has. Returns the new tween, to chain on.
    pub fn then(
        &mut self,
        previous: TweenId,
        entity: Entity,
        property: Property,
        to: Value,
        seconds: f32,
        ease: Ease,
    ) -> TweenId {
        let waiting = self.running.iter().any(|t| t.id == previous);
        self.push(
            entity,
            property,
            to,
            seconds,
            ease,
            waiting.then_some(previous),
        )
    }

    fn push(
        &mut self,
        entity: Entity,
        property: Property,
        to: Value,
        seconds: f32,
        ease: Ease,
        after: Option<TweenId>,
    ) -> TweenId {
        self.next += 1;
        let id = TweenId(self.next);
        if to.kind() != property.kind() || !to.is_finite() {
            log::warn!(
                "a tween of {} cannot go to {to:?}: not a value it takes",
                property.label()
            );
            self.finished.push(id);
            return id;
        }
        if after.is_none() {
            // Starting now, it takes the property over from any tween
            // already moving it, and from what was chained behind that one.
            let older: Vec<TweenId> = self
                .running
                .iter()
                .filter(|t| t.entity == entity && t.property == property && t.after.is_none())
                .map(|t| t.id)
                .collect();
            for old in older {
                self.cancel(old);
            }
        }
        self.running.push(Tween {
            id,
            entity,
            property,
            from: None,
            to,
            seconds: if seconds.is_finite() {
                seconds.max(0.0)
            } else {
                0.0
            },
            ease,
            elapsed: 0.0,
            after,
        });
        id
    }

    /// Stops `tween` where it is, and every tween chained behind it.
    pub fn cancel(&mut self, tween: TweenId) {
        let mut gone = vec![tween];
        while let Some(id) = gone.pop() {
            self.running.retain(|t| {
                if t.id == id || t.after == Some(id) {
                    if t.id != id {
                        gone.push(t.id);
                    }
                    false
                } else {
                    true
                }
            });
        }
    }

    /// Stops every tween on `entity`.
    pub fn cancel_all(&mut self, entity: Entity) {
        let ids: Vec<TweenId> = self
            .running
            .iter()
            .filter(|t| t.entity == entity)
            .map(|t| t.id)
            .collect();
        for id in ids {
            self.cancel(id);
        }
    }

    /// Whether `tween` is still to finish: running, or waiting its turn.
    pub fn is_running(&self, tween: TweenId) -> bool {
        self.running.iter().any(|t| t.id == tween)
    }

    /// Whether `tween` reached its value in the last step.
    pub fn finished(&self, tween: TweenId) -> bool {
        self.finished.contains(&tween)
    }

    pub fn len(&self) -> usize {
        self.running.len()
    }

    pub fn is_empty(&self) -> bool {
        self.running.is_empty()
    }

    /// Advances every tween by `delta` seconds and writes their values.
    pub fn step(&mut self, world: &mut World, delta: f32) {
        self.finished.clear();
        let delta = if delta.is_finite() {
            delta.max(0.0)
        } else {
            0.0
        };
        // Dead entities first, and the chains behind them.
        let dead: Vec<TweenId> = self
            .running
            .iter()
            .filter(|t| !world.is_alive(t.entity))
            .map(|t| t.id)
            .collect();
        for id in dead {
            self.cancel(id);
        }

        // In order, so a chain's next link starts in the step its previous
        // one finished and is handed the rest of the frame.
        let mut index = 0;
        while index < self.running.len() {
            let waiting = self.running[index]
                .after
                .is_some_and(|after| self.running.iter().any(|t| t.id == after));
            if waiting {
                index += 1;
                continue;
            }
            let tween = &mut self.running[index];
            tween.after = None;
            if tween.from.is_none() {
                let started = tween.property.read(world, tween.entity);
                let Some(from) = started else {
                    log::debug!(
                        "a tween of {} has no {} to move: dropped",
                        tween.property.label(),
                        tween.property.label()
                    );
                    let id = tween.id;
                    self.cancel(id);
                    continue;
                };
                tween.from = Some(from);
                // A chained link takes the property over when its turn
                // comes, as a new tween does when it is made.
                let (id, entity, property) = (tween.id, tween.entity, tween.property);
                let older: Vec<TweenId> = self
                    .running
                    .iter()
                    .filter(|t| {
                        t.id != id
                            && t.entity == entity
                            && t.property == property
                            && t.from.is_some()
                    })
                    .map(|t| t.id)
                    .collect();
                for old in older {
                    self.cancel(old);
                }
                // Cancelling may have moved this tween up the list.
                index = self
                    .running
                    .iter()
                    .position(|t| t.id == id)
                    .expect("a tween does not cancel itself");
            }
            let tween = &mut self.running[index];
            tween.elapsed += delta;
            let done = tween.elapsed >= tween.seconds;
            let value = match (done, tween.from) {
                (true, _) | (_, None) => tween.to,
                (false, Some(from)) => {
                    let t = tween.ease.apply(tween.elapsed / tween.seconds);
                    from.lerp(tween.to, t)
                }
            };
            tween.property.write(world, tween.entity, value);
            if done {
                let finished = self.running.remove(index);
                self.finished.push(finished.id);
                // Those waiting on it start in this step with what is left
                // of it. A link is always made after the one it follows, so
                // it is further down the list and stepped below, `delta`
                // added: what it is given here comes to the spare time.
                let spare = finished.elapsed - finished.seconds;
                for next in self
                    .running
                    .iter_mut()
                    .filter(|t| t.after == Some(finished.id))
                {
                    next.after = None;
                    next.elapsed = spare - delta;
                }
            } else {
                index += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests;
