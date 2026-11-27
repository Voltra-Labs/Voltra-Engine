//! Which cameras a frame draws, into what, in what order.

use voltra_assets::Handle;
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_render::{Camera2D, Texture};

use super::{placement, Camera, CameraTarget};

/// One camera's part of a frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shot {
    /// The camera's entity.
    pub entity: Entity,
    /// The camera component.
    pub camera: Camera,
    /// Where it is in the world.
    pub position: Vec2,
    /// How far it is turned in the world, counter-clockwise, in radians.
    pub rotation: f32,
    /// The render texture it draws into, or `None` for the screen.
    pub target: Option<Handle<Texture>>,
}

impl Shot {
    /// Its view on a target `width` × `height` pixels: the aspect theirs,
    /// turned as the camera is, and, pixel-perfect, the position moved by
    /// less than a pixel so the target's pixel edges fall on the world's
    /// pixel grid.
    ///
    /// **A turned view is not snapped**: its pixels run along other axes
    /// than the world's grid, so no shift lines the two up, and a sprite
    /// seen turned is resampled whatever the camera does. The snap comes
    /// back once the camera is upright again.
    pub fn view(&self, width: u32, height: u32) -> Camera2D {
        let (w, h) = (width.max(1) as f32, height.max(1) as f32);
        let mut position = self.position;
        let upright = !self.rotation.is_finite() || self.rotation == 0.0;
        if self.camera.pixel_perfect
            && upright
            && self.camera.size.is_finite()
            && self.camera.size > 0.0
        {
            // World units across one pixel of the target.
            let pixel = 2.0 * self.camera.size / h;
            // The view's left and bottom edges onto whole pixels.
            let half = Vec2::new(w, h) * 0.5 * pixel;
            let edge = position - half;
            position = (edge / pixel).round() * pixel + half;
        }
        Camera2D::new(position, self.camera.zoom(), w / h).with_rotation(self.rotation)
    }
}

/// The cameras a frame draws, in the order it draws them: every active
/// camera with a render texture first, then the screen's cameras, each set
/// by priority, lowest first — a later one drawn over an earlier where
/// their viewports overlap, as Unity's camera depth orders them.
///
/// A screen camera wholly under a later full-screen one is left out: it
/// would be drawn for nothing. A scene of several active full-screen
/// cameras draws only the highest, as it did when one camera took the
/// frame. A camera whose render texture is not resolved is left out too.
pub fn shots(world: &World) -> Vec<Shot> {
    let mut all: Vec<(Shot, bool, i32, u32)> = world
        .query::<Camera>()
        .filter(|(_, camera)| camera.active)
        .filter_map(|(entity, camera)| {
            let wants_texture = world.get::<CameraTarget>(entity);
            let target = match wants_texture {
                Some(t) => Some(t.handle?),
                None => None,
            };
            let (position, rotation) = placement(world, entity);
            let shot = Shot {
                entity,
                camera: *camera,
                position,
                rotation,
                target,
            };
            Some((shot, target.is_none(), camera.priority, entity.index()))
        })
        .collect();
    // Textures first, then by priority and index.
    all.sort_by_key(|(_, screen, priority, index)| (*screen, *priority, *index));
    let last_full = all
        .iter()
        .rposition(|(shot, screen, _, _)| *screen && shot.camera.viewport.clamped().is_full());
    all.into_iter()
        .enumerate()
        .filter(|(i, (_, screen, _, _))| !*screen || last_full.is_none_or(|full| *i >= full))
        .map(|(_, (shot, _, _, _))| shot)
        .collect()
}

#[cfg(test)]
mod tests {
    use voltra_assets::AssetPath;

    use super::*;
    use crate::camera::Viewport;
    use crate::{SceneId, Transform};

    fn camera(world: &mut World, camera: Camera, x: f32) -> Entity {
        let e = world.spawn();
        world.insert(e, SceneId::new());
        world.insert(e, Transform::from_translation(Vec2::new(x, 0.0)));
        world.insert(e, camera);
        e
    }

    #[test]
    fn of_several_full_screen_cameras_only_the_highest_draws() {
        let mut world = World::new();
        camera(&mut world, Camera::default(), 0.0);
        let top = camera(&mut world, Camera::default().with_priority(3), 1.0);
        let shots = shots(&world);
        assert_eq!(shots.len(), 1);
        assert_eq!(shots[0].entity, top);
    }

    #[test]
    fn split_screen_draws_both_halves_and_a_texture_camera_first() {
        let mut world = World::new();
        let left = camera(
            &mut world,
            Camera {
                viewport: Viewport::LEFT,
                ..Camera::default()
            },
            -5.0,
        );
        let right = camera(
            &mut world,
            Camera {
                viewport: Viewport::RIGHT,
                ..Camera::default()
            },
            5.0,
        );
        let map = camera(&mut world, Camera::new(20.0), 0.0);
        let mut target =
            CameraTarget::new(AssetPath::new("minimap.rendertexture.ron").expect("a path"));
        target.handle = Some(Handle::forge(9, 0));
        world.insert(map, target);
        let order: Vec<Entity> = shots(&world).iter().map(|s| s.entity).collect();
        assert_eq!(order, vec![map, left, right]);
    }

    #[test]
    fn an_unresolved_texture_camera_and_an_inactive_one_are_left_out() {
        let mut world = World::new();
        let map = camera(&mut world, Camera::default(), 0.0);
        world.insert(
            map,
            CameraTarget::new(AssetPath::new("m.rendertexture.ron").expect("a path")),
        );
        camera(
            &mut world,
            Camera {
                active: false,
                ..Camera::default()
            },
            0.0,
        );
        assert!(shots(&world).is_empty());
    }

    #[test]
    fn pixel_perfect_puts_the_edges_on_whole_pixels() {
        let mut world = World::new();
        let shot = Shot {
            entity: world.spawn(),
            camera: Camera {
                size: 2.0,
                pixel_perfect: true,
                ..Camera::default()
            },
            position: Vec2::new(0.013, -0.041),
            rotation: 0.0,
            target: None,
        };
        // 4 units over 64 pixels: a pixel is 1/16.
        let view = shot.view(65, 64);
        let pixel = 1.0 / 16.0;
        let left = view.position.x - 65.0 * 0.5 * pixel;
        let bottom = view.position.y - 32.0 * pixel;
        assert!(
            (left / pixel - (left / pixel).round()).abs() < 1e-4,
            "{left}"
        );
        assert!((bottom / pixel - (bottom / pixel).round()).abs() < 1e-4);
        assert!((view.position - shot.position).abs().max_element() <= pixel / 2.0 + 1e-6);
        let loose = Shot {
            camera: Camera::new(2.0),
            ..shot
        };
        assert_eq!(loose.view(65, 64).position, shot.position);
        let turned = Shot {
            rotation: 0.3,
            ..shot
        };
        let view = turned.view(65, 64);
        assert_eq!(view.position, shot.position, "a turned view is not snapped");
        assert_eq!(view.rotation, 0.3);
    }

    #[test]
    fn a_shot_turns_as_its_camera_does_in_the_world() {
        let mut world = World::new();
        let e = camera(&mut world, Camera::default(), 2.0);
        world
            .get_mut::<Transform>(e)
            .expect("spawned with one")
            .rotation = 0.5;
        let shots = shots(&world);
        assert!((shots[0].rotation - 0.5).abs() < 1e-6);
        assert!((shots[0].view(10, 10).rotation - 0.5).abs() < 1e-6);
    }
}
