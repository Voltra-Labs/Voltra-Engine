//! Every camera a frame draws, both dimensions, in the order it draws
//! them: texture shots producer before consumer, then the screen's by
//! priority with 3D first at a tie; each screen shot an overlay or not.

use voltra_assets::Handle;
use voltra_ecs::{Entity, World};
use voltra_render::Texture;

use super::{CameraTarget, Shot, Viewport};
use crate::camera3d::{Camera3D, CameraClear};

mod order;

use order::*;

/// One 3D camera's part of a frame: the twin of [`Shot`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shot3D {
    /// The camera's entity.
    pub entity: Entity,
    /// The camera component.
    pub camera: Camera3D,
    /// The render texture it draws into, or `None` for the screen.
    pub target: Option<Handle<Texture>>,
}

/// A shot of either dimension.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnyShot {
    /// A [`Camera2D`](super::Camera2D)'s.
    D2(Shot),
    /// A [`Camera3D`]'s.
    D3(Shot3D),
}

impl AnyShot {
    /// The camera's entity.
    pub fn entity(&self) -> Entity {
        match self {
            Self::D2(shot) => shot.entity,
            Self::D3(shot) => shot.entity,
        }
    }

    /// The render texture it draws into, or `None` for the screen.
    pub fn target(&self) -> Option<Handle<Texture>> {
        match self {
            Self::D2(shot) => shot.target,
            Self::D3(shot) => shot.target,
        }
    }

    /// The part of its target it draws.
    pub fn viewport(&self) -> Viewport {
        match self {
            Self::D2(shot) => shot.camera.viewport,
            Self::D3(shot) => shot.camera.viewport,
        }
    }

    /// Whether it is a 3D camera's.
    pub fn is_3d(&self) -> bool {
        matches!(self, Self::D3(_))
    }

    /// The draw order's key: priority, then 3D before 2D, then the entity.
    fn key(&self) -> (i32, bool, u32) {
        let priority = match self {
            Self::D2(shot) => shot.camera.priority,
            Self::D3(shot) => shot.camera.priority,
        };
        (priority, !self.is_3d(), self.entity().index())
    }
}

/// One shot of the frame, and whether it is an overlay.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameShot {
    /// The shot.
    pub shot: AnyShot,
    /// Whether its picture starts transparent: a screen shot that an
    /// earlier shot of the other dimension overlaps, in any part, and that
    /// is not a [`Camera3D`] cleared to a [`CameraClear::Color`].
    pub overlay: bool,
}

/// The frame's shots, in order, and the texture shots drawn before a shot
/// that fills a texture they may show — a cycle, so they would show its
/// last frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FrameShots {
    /// Every shot, in the order the frame draws them.
    pub shots: Vec<FrameShot>,
    /// The cameras drawn before a producer of a texture they may show: if
    /// they show it, it is last frame's picture.
    pub cycle: Vec<Entity>,
}

impl FrameShots {
    /// Whether any shot is a 3D camera's.
    pub fn has_3d(&self) -> bool {
        self.shots.iter().any(|s| s.shot.is_3d())
    }

    /// The screen's shots, in order.
    pub fn screen(&self) -> impl Iterator<Item = &FrameShot> {
        self.shots.iter().filter(|s| s.shot.target().is_none())
    }
}

/// Every active 3D camera's shot: those with a render texture, then the
/// screen's — each set by priority, then entity — less a screen camera
/// wholly under a later full-screen one, and one whose render texture is
/// not resolved. [`shots`](super::shots)'s rule, among 3D cameras only.
pub fn shots_3d(world: &World) -> Vec<Shot3D> {
    let all = candidates_3d(world);
    let last_full = all
        .iter()
        .rposition(|s| s.target.is_none() && s.camera.viewport.clamped().is_full());
    all.into_iter()
        .enumerate()
        .filter(|(i, s)| s.target.is_some() || last_full.is_none_or(|full| *i >= full))
        .map(|(_, s)| s)
        .collect()
}

/// Every active 3D camera's shot, none pruned, in [`shots_3d`]'s order.
fn candidates_3d(world: &World) -> Vec<Shot3D> {
    let mut all: Vec<Shot3D> = world
        .query::<Camera3D>()
        .filter(|(_, camera)| camera.active)
        .filter_map(|(entity, camera)| {
            let target = match world.get::<CameraTarget>(entity) {
                Some(t) => Some(t.handle?),
                None => None,
            };
            Some(Shot3D {
                entity,
                camera: *camera,
                target,
            })
        })
        .collect();
    all.sort_by_key(|s| (s.target.is_none(), s.camera.priority, s.entity.index()));
    all
}

/// The frame of `world` on a `screen` of that many pixels: every camera
/// of both dimensions in draw order.
///
/// - **Texture shots first**, a producer before every shot that may show
///   its texture: a 2D camera may show every render texture a world sprite
///   names (but its own); a 3D camera none yet (surface materials, stage
///   137). Otherwise by priority, 3D first at a tie. Where producers may
///   show each other's textures the order cannot hold for all: a producer
///   inside the cycle draws first, with the other's last frame, and is
///   named in [`cycle`](FrameShots::cycle).
/// - **Then the screen's**, by priority, 3D first at a tie, then entity. A
///   camera wholly under a later full-screen camera of its own dimension
///   with no camera of the other between them is left out — several
///   framings of one world, the highest wins, as [`shots`](super::shots) prunes — but a
///   camera of the other dimension between them keeps it: its world is a
///   layer of the picture, not a framing replaced.
/// - **Overlay or framing**: a screen shot overlapped, by a pixel or more,
///   by an earlier one of the other dimension is an overlay, its picture
///   cleared transparent — Unity's overlay camera, Godot's canvas over its
///   world — unless it is a 3D camera with a colour of its own to clear to.
pub fn frame_shots(world: &World, screen: (u32, u32)) -> FrameShots {
    let mut textures = Vec::new();
    let mut on_screen = Vec::new();
    let all = super::shots::candidates(world)
        .into_iter()
        .map(AnyShot::D2)
        .chain(candidates_3d(world).into_iter().map(AnyShot::D3));
    for shot in all {
        if shot.target().is_some() {
            textures.push(shot);
        } else {
            on_screen.push(shot);
        }
    }
    textures.sort_by_key(AnyShot::key);
    on_screen.sort_by_key(AnyShot::key);
    let shown = shown_by_2d(world);
    let (textures, cycle) = producers_first(textures, |shot| match shot {
        AnyShot::D2(shot) => shown
            .iter()
            .copied()
            .filter(|&t| Some(t) != shot.target)
            .collect(),
        AnyShot::D3(_) => Vec::new(),
    });
    let mut out: Vec<FrameShot> = textures
        .into_iter()
        .map(|shot| FrameShot {
            shot,
            overlay: false,
        })
        .collect();
    let on_screen = prune(on_screen);
    for (i, &shot) in on_screen.iter().enumerate() {
        let opaque = matches!(
            shot,
            AnyShot::D3(Shot3D {
                camera: Camera3D {
                    clear: CameraClear::Color(_),
                    ..
                },
                ..
            })
        );
        let under = on_screen[..i].iter().any(|earlier| {
            earlier.is_3d() != shot.is_3d() && overlaps(earlier.viewport(), shot.viewport(), screen)
        });
        out.push(FrameShot {
            shot,
            overlay: under && !opaque,
        });
    }
    FrameShots { shots: out, cycle }
}
