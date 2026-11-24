//! Bodies at rest stop being simulated until something disturbs them.
//!
//! Box2D's rule. A dynamic body slower than a tolerance accumulates rest
//! time; an *island* — dynamic bodies joined by contacts — goes to sleep
//! together once every member has rested long enough, because one body
//! asleep under an awake one would be a support that no longer pushes back.
//! A sleeping body is not integrated and its contacts are not solved, which
//! removes both their cost and the millimetre of jitter a settled stack keeps.
//!
//! It wakes, with its whole island, when anything says it is no longer at
//! rest: a velocity written by the game, a transform moved from outside the
//! step, contact with an awake body or a moving kinematic one, losing a
//! support it was resting on, or its body changing type or opting out.
//!
//! Sleep is simulation state, kept here, not on the component: a scene file
//! does not say a crate is asleep, and a reload starts everything awake.

use crate::hash::{FastMap, FastSet};

use voltra_ecs::{Entity, World};
use voltra_scene::{BodyType, Collider, RigidBody, Transform};

use crate::narrow::Contact;

/// When a body counts as at rest, and how long it must stay so.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SleepParams {
    /// Off simulates everything every step, as before sleeping existed.
    pub enabled: bool,
    /// World units per second below which a body is at rest.
    pub linear_tolerance: f32,
    /// Radians per second below which a body is at rest.
    pub angular_tolerance: f32,
    /// Seconds an island must rest before it sleeps.
    pub time_to_sleep: f32,
}

impl Default for SleepParams {
    /// 10 cm/s, 5 °/s, half a second.
    ///
    /// Twice Box2D v3's linear tolerance, which assumes a solver quieter
    /// than this one: a settled 20-row pyramid here keeps about 6 cm/s of
    /// jitter and would never sleep at 5. 10 cm/s is Unity's default — its
    /// kinetic-energy threshold of 0.005 per unit mass — and still far below
    /// anything that reads as motion.
    fn default() -> Self {
        Self {
            enabled: true,
            linear_tolerance: 0.1,
            angular_tolerance: 5.0_f32.to_radians(),
            time_to_sleep: 0.5,
        }
    }
}

/// How a sleeping body was left, to tell whether it has been disturbed.
#[derive(Debug, Clone, PartialEq)]
struct Rest {
    at: Transform,
    /// Everything it was in contact with, and where each was: a support that
    /// goes away, moves or wakes wakes it rather than leaving it hanging.
    ///
    /// Compared by the partner's own state, never by whether the contact is
    /// still reported. Two boxes resting edge to edge flicker in and out of
    /// contact from one step to the next; waking on that would keep a whole
    /// level of crates awake for nothing.
    partners: Vec<(Entity, Option<Transform>)>,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct BodySleep {
    /// Seconds below the tolerances, while awake.
    rested: f32,
    asleep: Option<Rest>,
}

/// Every dynamic body's rest time and, for the sleeping ones, how they were
/// left.
#[derive(Debug, Clone, Default)]
pub struct Sleep {
    bodies: FastMap<Entity, BodySleep>,
    /// The last contact of every pair with nothing awake in it, reused in
    /// place of a narrow-phase test: nothing in such a pair has moved, so
    /// the manifold cannot have changed. Box2D keeps sleeping contacts the
    /// same way, and it is most of what sleeping saves.
    resting: FastMap<(Entity, Entity), Contact>,
}

impl Sleep {
    /// Whether `entity` is asleep.
    pub fn is_asleep(&self, entity: Entity) -> bool {
        self.bodies
            .get(&entity)
            .is_some_and(|body| body.asleep.is_some())
    }

    /// How many bodies are asleep.
    pub fn asleep_count(&self) -> usize {
        self.bodies.values().filter(|b| b.asleep.is_some()).count()
    }

    /// Every sleeping body.
    pub fn asleep(&self) -> impl Iterator<Item = Entity> + '_ {
        self.bodies
            .iter()
            .filter(|(_, b)| b.asleep.is_some())
            .map(|(e, _)| *e)
    }

    /// Wakes `entity` now. Its island follows on the next step, through the
    /// contacts it shares with it.
    pub fn wake(&mut self, entity: Entity) {
        if let Some(body) = self.bodies.get_mut(&entity) {
            *body = BodySleep::default();
        }
    }

    /// Forgets every body: everything starts awake.
    pub fn clear(&mut self) {
        self.bodies.clear();
        self.resting.clear();
    }

    /// Whether `entity` is a dynamic body that is not asleep — the only kind
    /// a solved contact needs on at least one side.
    pub fn is_awake_dynamic(&self, world: &World, entity: Entity) -> bool {
        !self.is_asleep(entity) && is_dynamic(world, entity)
    }

    /// The contact `(a, b)` rested with, if the pair is at rest and was
    /// touching: what the collide reuses instead of testing again.
    pub fn resting_contact(&self, world: &World, a: Entity, b: Entity) -> Option<&Contact> {
        if self.is_awake_dynamic(world, a) || self.is_awake_dynamic(world, b) {
            return None;
        }
        self.resting.get(&(a, b))
    }

    /// Remembers a resting contact for the next step to reuse, unless it is
    /// already the one remembered.
    pub fn remember_resting(&mut self, contact: &Contact) {
        self.resting
            .entry((contact.a, contact.b))
            .or_insert(*contact);
    }

    /// Forgets every remembered contact not in `pairs` — this step's resting
    /// set — so a pair that woke, separated or was despawned is tested again
    /// rather than reused.
    pub fn keep_resting(&mut self, pairs: &FastSet<(Entity, Entity)>) {
        self.resting.retain(|pair, _| pairs.contains(pair));
    }

    /// Wakes every sleeping body whose own state says it was disturbed —
    /// moved, given a velocity, retyped, opted out, or its support gone.
    /// Run before the collide, so a disturbed body's contacts are tested
    /// rather than reused.
    pub fn wake_disturbed(&mut self, world: &World) {
        let mut woken: Vec<Entity> = Vec::new();
        self.bodies.retain(|&entity, sleep| {
            let Some(body) = world.get::<RigidBody>(entity) else {
                return false;
            };
            let Some(rest) = &sleep.asleep else {
                return true;
            };
            let moved = world.get::<Transform>(entity) != Some(&rest.at);
            // A partner that woke and stayed is still in contact, and the
            // island pass wakes this one through it; one that woke and left
            // has moved, which its stored transform catches.
            let lost_support = rest.partners.iter().any(|(partner, was_at)| {
                world.get::<Collider>(*partner).is_none()
                    || world.get::<Transform>(*partner) != was_at.as_ref()
            });
            let disturbed = body.body_type != BodyType::Dynamic
                || !body.can_sleep
                || body.velocity != voltra_render::glam::Vec2::ZERO
                || body.angular_velocity != 0.0
                || moved
                || lost_support;
            if disturbed {
                woken.push(entity);
            }
            true
        });
        for entity in woken {
            self.wake(entity);
        }
    }

    /// Wakes every sleeping body in an island with something awake in it.
    /// Run after the collide, against all of this step's contacts.
    pub fn wake_islands(&mut self, world: &World, contacts: &[(Entity, Entity)]) {
        // Seeds: sleeping bodies touching an awake dynamic body or a moving
        // kinematic one. With none — the usual step of a settled scene —
        // nothing is woken and no island needs building.
        let mut seeds: Vec<Entity> = Vec::new();
        for &(a, b) in contacts {
            let (a_asleep, b_asleep) = (self.is_asleep(a), self.is_asleep(b));
            if a_asleep == b_asleep {
                continue;
            }
            let (sleeper, other) = if a_asleep { (a, b) } else { (b, a) };
            if is_dynamic(world, other) || is_moving_kinematic(world, other) {
                seeds.push(sleeper);
            }
        }
        if seeds.is_empty() {
            return;
        }

        // Then everything asleep joined to a seed by contacts between sleeping
        // bodies: the rest of its island.
        let mut joined: FastMap<Entity, Vec<Entity>> = FastMap::default();
        for &(a, b) in contacts {
            if self.is_asleep(a)
                && self.is_asleep(b)
                && is_dynamic(world, a)
                && is_dynamic(world, b)
            {
                joined.entry(a).or_default().push(b);
                joined.entry(b).or_default().push(a);
            }
        }
        while let Some(entity) = seeds.pop() {
            if !self.is_asleep(entity) {
                continue;
            }
            self.wake(entity);
            if let Some(next) = joined.get(&entity) {
                seeds.extend(next.iter().copied());
            }
        }
    }

    /// Accumulates rest for every awake dynamic body and puts to sleep each
    /// island that has rested long enough. Run after the solve, against the
    /// contacts that were solved.
    pub fn settle(
        &mut self,
        world: &mut World,
        contacts: &[(Entity, Entity)],
        driven: &[Entity],
        params: &SleepParams,
        dt: f32,
    ) {
        if !params.enabled {
            self.clear();
            return;
        }
        let linear = params.linear_tolerance * params.linear_tolerance;
        let angular = params.angular_tolerance * params.angular_tolerance;

        for (entity, body) in world.query::<RigidBody>() {
            if body.body_type != BodyType::Dynamic {
                self.bodies.remove(&entity);
                continue;
            }
            let sleep = self.bodies.entry(entity).or_default();
            if sleep.asleep.is_some() {
                continue;
            }
            let at_rest = body.can_sleep
                && body.velocity.length_squared() <= linear
                && body.angular_velocity * body.angular_velocity <= angular;
            sleep.rested = if at_rest { sleep.rested + dt } else { 0.0 };
        }

        let mut islands = Islands::from_contacts(world, contacts);
        // A body a joint's motor turns is driven, as one a moving platform
        // carries is: its island stays awake.
        for &entity in driven {
            islands.driven.insert(islands.root(entity));
        }
        // The least-rested member decides for its island; a driven island
        // never sleeps.
        let mut least: FastMap<Entity, f32> = FastMap::default();
        for (&entity, sleep) in &self.bodies {
            if sleep.asleep.is_some() {
                continue;
            }
            let rested = if islands.is_driven(entity) {
                0.0
            } else {
                sleep.rested
            };
            let root = islands.root(entity);
            let slot = least.entry(root).or_insert(f32::INFINITY);
            *slot = slot.min(rested);
        }

        let partners = partners_of(contacts);
        let falling_asleep: Vec<Entity> = self
            .bodies
            .iter()
            .filter(|(entity, sleep)| {
                sleep.asleep.is_none()
                    && least
                        .get(&islands.root(**entity))
                        .is_some_and(|&rested| rested >= params.time_to_sleep)
            })
            .map(|(entity, _)| *entity)
            .collect();
        for entity in falling_asleep {
            let Some(at) = world.get::<Transform>(entity).copied() else {
                continue;
            };
            if let Some(body) = world.get_mut::<RigidBody>(entity) {
                body.velocity = voltra_render::glam::Vec2::ZERO;
                body.angular_velocity = 0.0;
            }
            let mut partners: Vec<(Entity, Option<Transform>)> = partners
                .get(&entity)
                .map(|set| {
                    set.iter()
                        .map(|p| (*p, world.get::<Transform>(*p).copied()))
                        .collect()
                })
                .unwrap_or_default();
            partners.sort_unstable_by_key(|(e, _)| e.index());
            if let Some(sleep) = self.bodies.get_mut(&entity) {
                sleep.asleep = Some(Rest { at, partners });
            }
        }
    }
}

/// Every entity each one is in solid contact with.
fn partners_of(contacts: &[(Entity, Entity)]) -> FastMap<Entity, FastSet<Entity>> {
    let mut partners: FastMap<Entity, FastSet<Entity>> = FastMap::default();
    for &(a, b) in contacts {
        partners.entry(a).or_default().insert(b);
        partners.entry(b).or_default().insert(a);
    }
    partners
}

fn is_dynamic(world: &World, entity: Entity) -> bool {
    world
        .get::<RigidBody>(entity)
        .is_some_and(|b| b.body_type == BodyType::Dynamic)
}

fn is_moving_kinematic(world: &World, entity: Entity) -> bool {
    world.get::<RigidBody>(entity).is_some_and(|b| {
        b.body_type == BodyType::Kinematic
            && (b.velocity != voltra_render::glam::Vec2::ZERO || b.angular_velocity != 0.0)
    })
}

/// Dynamic bodies joined by contacts, as a union-find. Static and kinematic
/// bodies join nothing — a floor under two separate stacks does not make them
/// one island — but a kinematic one that is moving *drives* whatever it
/// touches.
struct Islands {
    parent: FastMap<Entity, Entity>,
    driven: FastSet<Entity>,
}

impl Islands {
    fn from_contacts(world: &World, contacts: &[(Entity, Entity)]) -> Self {
        let mut islands = Self {
            parent: FastMap::default(),
            driven: FastSet::default(),
        };
        for &(a, b) in contacts {
            let (a_dynamic, b_dynamic) = (is_dynamic(world, a), is_dynamic(world, b));
            if a_dynamic && b_dynamic {
                islands.union(a, b);
            }
            if a_dynamic && is_moving_kinematic(world, b) {
                islands.driven.insert(a);
            }
            if b_dynamic && is_moving_kinematic(world, a) {
                islands.driven.insert(b);
            }
        }
        // Every entry pointed straight at its root, once, so each later
        // `root` is one lookup rather than a walk down a chain as long as
        // the island — a settled pyramid is a single island of hundreds.
        let roots: Vec<(Entity, Entity)> = islands
            .parent
            .keys()
            .map(|&entity| (entity, islands.root(entity)))
            .collect();
        islands.parent.extend(roots);
        // Driven is a property of the island, not of the one body touching.
        let driven: Vec<Entity> = islands.driven.iter().copied().collect();
        islands.driven = driven.into_iter().map(|e| islands.root(e)).collect();
        islands
    }

    fn root(&self, entity: Entity) -> Entity {
        let mut cursor = entity;
        // Bounded by the number of entries: every step moves towards a root.
        for _ in 0..=self.parent.len() {
            match self.parent.get(&cursor) {
                Some(&next) if next != cursor => cursor = next,
                _ => break,
            }
        }
        cursor
    }

    fn union(&mut self, a: Entity, b: Entity) {
        let (ra, rb) = (self.root(a), self.root(b));
        // Path halving on the way: the two walks just made are shortened, so
        // building the islands stays near-linear in the contacts.
        self.parent.insert(a, ra);
        self.parent.insert(b, rb);
        self.parent.entry(a).or_insert(a);
        self.parent.entry(b).or_insert(b);
        if ra != rb {
            // The lower index becomes the root, so the result does not
            // depend on contact order.
            let (keep, merge) = if ra.index() <= rb.index() {
                (ra, rb)
            } else {
                (rb, ra)
            };
            self.parent.insert(merge, keep);
            self.parent.entry(keep).or_insert(keep);
        }
    }

    fn is_driven(&self, entity: Entity) -> bool {
        self.driven.contains(&self.root(entity))
    }
}
