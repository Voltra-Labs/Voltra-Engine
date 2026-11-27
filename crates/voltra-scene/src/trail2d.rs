//! A ribbon left behind a moving entity, fading as it ages: a sword's
//! swing, a comet's tail, a car's skid.
//!
//! Unity's `TrailRenderer` and Godot's trail recipe (a `Line2D` fed points
//! from a script), as one component: while the world runs, the entity's
//! world position is dropped as a point each time it has moved
//! `min_distance`, every point ages, and one older than `lifetime` is
//! gone. The ribbon runs from where the entity is now through the points,
//! newest first, widened by the same [`stroke`](crate::line2d::stroke) a
//! `Line2D` is — its width and colour by age rather than by length.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use voltra_assets::{AssetPath, Handle, Textures};
use voltra_ecs::World;
use voltra_render::glam::Vec2;
use voltra_render::Texture;

use crate::batch::TriangleMesh;
use crate::curve::Curve;
use crate::gradient::Gradient;
use crate::hierarchy::WorldTransforms2D;
use crate::line2d::{stroke, LineCap, LineJoint, LineTexture, Stroke};

/// The longest step a trail ages by at once, as particles: a stalled frame
/// does not wipe a trail.
const MAX_STEP: f32 = 0.1;

/// A ribbon behind its entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trail2D {
    /// Seconds a point of the trail lasts.
    pub lifetime: f32,
    /// Full width at the entity, in world units.
    pub width: f32,
    /// What the width is multiplied by over a point's age, `0` new to `1`
    /// at `lifetime`. `None` tapers it straight to nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_curve: Option<Curve>,
    /// Linear RGBA at the entity.
    pub color: [f32; 4],
    /// The colour over a point's age, in place of `color` fading to clear
    /// when it has a stop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradient: Option<Gradient>,
    /// How far, in world units, the entity moves before a new point is
    /// dropped.
    pub min_distance: f32,
    /// The most points kept; the oldest go first.
    pub max_points: u32,
    /// The image along the ribbon, by path, stretched from the entity to
    /// the tail. `None` is a flat colour.
    #[serde(default)]
    pub texture: Option<AssetPath>,
    /// What `texture` resolves to in this session's textures. Never written.
    #[serde(skip)]
    pub texture_handle: Option<Handle<Texture>>,
    /// Draw order among sprites. See [`Sprite2D::sort_order`](crate::Sprite2D::sort_order).
    #[serde(default)]
    pub sort_order: i32,
    /// The named layer it draws on. See [`SortingLayers`](crate::sorting::SortingLayers).
    #[serde(
        default = "crate::sorting::default_layer",
        skip_serializing_if = "crate::sorting::is_default_layer"
    )]
    pub sorting_layer: String,
    /// The light layers it is on, as a sprite's are.
    #[serde(
        default = "crate::light::default_light_layers",
        skip_serializing_if = "crate::light::is_default_light_layers"
    )]
    pub light_layers: u32,
    /// The points dropped so far: where, and how old, newest first. Never
    /// saved — a scene says what a trail is, not where it has been.
    #[serde(skip)]
    pub points: VecDeque<(Vec2, f32)>,
}

impl Default for Trail2D {
    /// Half a second of white ribbon a fifth of a unit wide.
    fn default() -> Self {
        Self {
            lifetime: 0.5,
            width: 0.2,
            width_curve: None,
            color: [1.0; 4],
            gradient: None,
            min_distance: 0.05,
            max_points: 256,
            texture: None,
            texture_handle: None,
            sort_order: 0,
            sorting_layer: crate::sorting::default_layer(),
            light_layers: crate::light::default_light_layers(),
            points: VecDeque::new(),
        }
    }
}

impl Trail2D {
    /// The registered name a scene file stores it under.
    pub const NAME: &'static str = "Trail2D";

    /// Ages every point by `dt`, drops the dead, and drops a new point at
    /// `head` if the entity has moved far enough from the last one.
    pub fn advance(&mut self, head: Vec2, dt: f32) {
        if !head.is_finite() {
            return;
        }
        let lifetime = if self.lifetime.is_finite() {
            self.lifetime.max(0.0)
        } else {
            0.0
        };
        for (_, age) in &mut self.points {
            *age += dt;
        }
        while self.points.back().is_some_and(|(_, age)| *age >= lifetime) {
            self.points.pop_back();
        }
        let far = self
            .points
            .front()
            .is_none_or(|(last, _)| last.distance(head) >= self.min_distance.max(0.0));
        if far && lifetime > 0.0 {
            self.points.push_front((head, 0.0));
        }
        self.points.truncate(self.max_points.max(1) as usize);
    }

    /// Forgets where it has been: after a teleport, or a respawn.
    pub fn clear(&mut self) {
        self.points.clear();
    }

    /// The ribbon from `head` — where the entity is now — through the
    /// points, in world space.
    pub fn mesh(&self, head: Vec2) -> TriangleMesh {
        let lifetime = self.lifetime;
        if lifetime.is_nan() || lifetime <= 0.0 || !head.is_finite() {
            return TriangleMesh::default();
        }
        let mut path = vec![(head, 0.0)];
        path.extend(self.points.iter().copied());
        let points: Vec<Vec2> = path.iter().map(|(p, _)| *p).collect();
        // Ages by length along, so the stroke's `t` can be read as an age.
        let mut along = vec![0.0f32];
        for pair in points.windows(2) {
            along.push(along[along.len() - 1] + pair[0].distance(pair[1]));
        }
        let total = along[along.len() - 1];
        let age_at = |t: f32| -> f32 {
            let s = t * total;
            let i = along.partition_point(|&a| a <= s).clamp(1, along.len() - 1);
            let span = along[i] - along[i - 1];
            let f = if span > 0.0 {
                (s - along[i - 1]) / span
            } else {
                0.0
            };
            let age = path[i - 1].1 + (path[i].1 - path[i - 1].1) * f;
            (age / lifetime).clamp(0.0, 1.0)
        };
        let width = |t: f32| {
            let a = age_at(t);
            self.width * self.width_curve.as_ref().map_or(1.0 - a, |c| c.sample(a))
        };
        let color = |t: f32| {
            let a = age_at(t);
            self.gradient.as_ref().and_then(|g| g.sample(a)).unwrap_or([
                self.color[0],
                self.color[1],
                self.color[2],
                self.color[3] * (1.0 - a),
            ])
        };
        stroke(&Stroke {
            points: &points,
            closed: false,
            width: &width,
            color: &color,
            joint: LineJoint::Round,
            miter_limit: 2.0,
            caps: (LineCap::Round, LineCap::Butt),
            texture: LineTexture::Stretch,
            tile: self.width,
        })
    }

    /// Sets or clears the texture path and refreshes its handle, as
    /// [`Sprite2D::set_texture`](crate::Sprite2D::set_texture) does.
    pub fn set_texture(
        &mut self,
        path: Option<AssetPath>,
        textures: &mut Textures,
        device: &voltra_render::wgpu::Device,
        queue: &voltra_render::wgpu::Queue,
    ) {
        self.texture_handle = path.as_ref().map(|path| textures.load(device, queue, path));
        self.texture = path;
    }
}

/// Advances every trail by `dt` from where its entity now is in the world.
/// A step that is not finite or not positive does nothing.
pub fn advance(world: &mut World, dt: f32) {
    if !dt.is_finite() || dt <= 0.0 {
        return;
    }
    let dt = dt.min(MAX_STEP);
    let transforms = WorldTransforms2D::from_world(world);
    for (entity, trail) in world.query_mut::<Trail2D>() {
        let head = transforms.matrix(entity).transform_point2(Vec2::ZERO);
        trail.advance(head, dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_are_dropped_as_it_moves_and_die_of_age() {
        let mut trail = Trail2D {
            lifetime: 0.5,
            min_distance: 0.1,
            ..Trail2D::default()
        };
        for i in 0..10 {
            trail.advance(Vec2::new(i as f32 * 0.25, 0.0), 0.125);
        }
        // Points live half a second: the last four steps' worth.
        assert_eq!(trail.points.len(), 4);
        assert_eq!(trail.points[0].0, Vec2::new(2.25, 0.0), "newest first");
        trail.advance(Vec2::new(2.3, 0.0), 0.125);
        assert_eq!(
            trail.points[0].0,
            Vec2::new(2.25, 0.0),
            "too close to drop one"
        );
    }

    #[test]
    fn the_ribbon_tapers_and_fades_towards_its_tail() {
        let mut trail = Trail2D {
            lifetime: 1.0,
            width: 1.0,
            min_distance: 0.0,
            ..Trail2D::default()
        };
        for i in 0..10 {
            trail.advance(Vec2::new(-(9 - i) as f32 * 0.5, 0.0), 0.1);
        }
        let mesh = trail.mesh(Vec2::ZERO);
        assert!(!mesh.is_empty());
        let near_head = mesh
            .vertices
            .iter()
            .filter(|v| v.position.x > -0.5)
            .map(|v| v.color[3])
            .fold(0.0, f32::max);
        let near_tail = mesh
            .vertices
            .iter()
            .filter(|v| v.position.x < -3.5)
            .map(|v| v.color[3])
            .fold(0.0, f32::max);
        assert!(
            near_head > 0.8 && near_tail < 0.3,
            "{near_head} then {near_tail}"
        );
    }

    #[test]
    fn a_trail_that_has_not_moved_draws_nothing_and_survives_nonsense() {
        let mut trail = Trail2D::default();
        assert!(trail.mesh(Vec2::ZERO).is_empty());
        trail.advance(Vec2::NAN, 0.1);
        assert!(trail.points.is_empty());
        let text = ron::to_string(&trail).expect("writes");
        assert!(!text
            .split([',', '('])
            .any(|field| field.starts_with("points:")));
    }
}
