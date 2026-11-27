//! Following a target, or framing several: smoothed, with a dead zone, a
//! lead, limits and a zoom.

mod group;
mod limits;
mod step;
mod zoom;

pub(super) use step::update;
pub use zoom::FollowZoom;

use serde::{Deserialize, Serialize};
use voltra_render::glam::Vec2;

use crate::format::SceneRefs;
use crate::scene_id::SceneId;

/// Keeps the entity — usually a camera — on `target`, or on the middle of
/// the box around `target` and every one of `targets`: Cinemachine's
/// Target Group, a group of one being one target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CameraFollow2D {
    /// What it follows, by identity; `None` follows nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<SceneId>,
    /// More it frames with `target`, by identity: it aims at the middle of
    /// the box around all of them. Empty — the default, and not written —
    /// is `target` alone. A target that leaves the scene drops out of the
    /// box; with none left, it holds still.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<SceneId>,
    /// Where it sits from the target, in world units: a camera that shows
    /// more of what is above the player than below.
    pub offset: Vec2,
    /// How fast it closes on where it should be: the share of the distance
    /// left closed per second, as an exponential rate, so the frame rate
    /// does not change where it ends up. Zero snaps.
    pub smoothing: f32,
    /// Half the size of the box, around its own centre, the target moves in
    /// without moving it — Godot's drag margins, Cinemachine's dead zone —
    /// in world units.
    pub dead_zone: Vec2,
    /// Seconds of the target's velocity it looks ahead by. Zero looks at
    /// the target.
    pub look_ahead: f32,
    /// What a camera on it may show, or where any other entity may be: kept
    /// inside, its view and all. `None` is unbounded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<FollowBounds>,
    /// Zooms a camera on it so its targets fit, with a margin, between two
    /// sizes. `None` — the default, and not written — keeps the camera's
    /// size; on an entity without a [`Camera2D`] it does nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zoom: Option<FollowZoom>,
    /// What it remembers from frame to frame. Never saved.
    #[serde(skip)]
    pub state: FollowState,
}

/// A follower's memory between frames.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FollowState {
    /// Where the target — the group's middle — was last frame, for its
    /// velocity; `None` snaps on the next frame.
    last_target: Option<Vec2>,
    lead: Vec2,
    /// Which of the group were there last frame, hashed: a change moves
    /// the middle without the group moving.
    members: u64,
    /// The targets already said missing, so each is said once.
    missing: Vec<SceneId>,
    /// Whether it named itself as a target last frame, so that is said once.
    named_itself: bool,
}

/// A rectangle in the world.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FollowBounds {
    /// The corner with the smallest x and y, in world units.
    pub min: Vec2,
    /// The corner with the largest x and y, in world units.
    pub max: Vec2,
}

impl Default for CameraFollow2D {
    fn default() -> Self {
        Self {
            target: None,
            targets: Vec::new(),
            offset: Vec2::ZERO,
            smoothing: 5.0,
            dead_zone: Vec2::ZERO,
            look_ahead: 0.0,
            bounds: None,
            zoom: None,
            state: FollowState::default(),
        }
    }
}

impl CameraFollow2D {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "CameraFollow2D";

    /// Following `target`, with the defaults.
    pub fn new(target: SceneId) -> Self {
        Self {
            target: Some(target),
            ..Self::default()
        }
    }

    /// Framing every one of `targets` — the first as [`Self::target`] —
    /// with the defaults.
    pub fn framing(targets: impl IntoIterator<Item = SceneId>) -> Self {
        let mut targets = targets.into_iter();
        Self {
            target: targets.next(),
            targets: targets.collect(),
            ..Self::default()
        }
    }

    /// Everything it frames: `target`, then `targets`, each once.
    pub fn group(&self) -> Vec<SceneId> {
        let mut group: Vec<SceneId> = Vec::with_capacity(self.targets.len() + 1);
        for id in self.target.iter().chain(&self.targets) {
            if !group.contains(id) {
                group.push(*id);
            }
        }
        group
    }

    /// Lands on the target next frame instead of travelling to it — after
    /// a teleport, a scene change, a new target.
    pub fn snap(&mut self) {
        self.state.last_target = None;
        self.state.lead = Vec2::ZERO;
    }
}

impl SceneRefs for CameraFollow2D {
    fn remap(&mut self, map: &dyn Fn(SceneId) -> Option<SceneId>) {
        if let Some(id) = self.target {
            self.target = Some(map(id).unwrap_or(id));
        }
        for id in &mut self.targets {
            *id = map(*id).unwrap_or(*id);
        }
    }
}
