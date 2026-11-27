//! Painting a skinned sprite's weights in the viewport.
//!
//! **A mode of the viewport while the skinned sprite is selected**, as
//! editing a shadow shape is: the inspector turns it on and picks the bone;
//! while it is on, a primary drag adds that bone's weight under a soft round
//! brush, a drag with Shift held takes it away, and every vertex is drawn
//! as a dot from blue (none of the bone) to red (all of it). `Esc`, the
//! button again or selecting anything else turns it off. Unity's Sprite
//! Skinning Editor paints the same way.
//!
//! The brush works on the mesh as it is drawn — bent by the pose — and
//! changes the weights of the vertices under it; the weights are the
//! component's, so a stroke is an edit of the selected entity and the
//! history makes one entry of each.

use voltra_core::egui::{Key, Response, Ui};
use voltra_core::UiFrame;
use voltra_ecs::{Entity, World};
use voltra_render::glam::Vec2;
use voltra_scene::hierarchy::world_matrix_2d;
use voltra_scene::skinned::{pose, weights};
use voltra_scene::SkinnedSprite;

/// What the history entry of a stroke is called.
const LABEL: &str = "Paint weights";

/// Which skinned sprite is being painted, with which bone and brush.
#[derive(Debug)]
pub struct WeightPaint {
    painting: Option<Entity>,
    /// The bone painted, an index into the component's bone list.
    pub bone: u32,
    /// The brush's radius, in world units.
    pub radius: f32,
    /// How much weight a second of dragging adds at the brush's middle.
    pub strength: f32,
}

impl Default for WeightPaint {
    fn default() -> Self {
        Self {
            painting: None,
            bone: 0,
            radius: 0.25,
            strength: 2.0,
        }
    }
}

impl WeightPaint {
    /// Whether `entity` is being painted.
    pub fn is_painting(&self, entity: Entity) -> bool {
        self.painting == Some(entity)
    }

    /// Paints `entity`'s weights, or stops for `None`.
    pub fn set_painting(&mut self, entity: Option<Entity>) {
        self.painting = entity;
    }

    /// The skinned sprite being painted, if it is still the selection and
    /// still has the component; painting stops otherwise.
    pub fn target(&mut self, world: &World, selected: Option<Entity>) -> Option<Entity> {
        let entity = self.painting?;
        let still = selected == Some(entity) && world.get::<SkinnedSprite>(entity).is_some();
        if !still {
            self.painting = None;
        }
        still.then_some(entity)
    }

    /// One frame of the brush on `entity`. Returns the history's label while
    /// a stroke is live.
    pub fn update(
        &mut self,
        ui: &Ui,
        scene: &Response,
        frame: &mut UiFrame<'_>,
        entity: Entity,
    ) -> Option<&'static str> {
        if !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.key_pressed(Key::Escape)) {
            self.painting = None;
            return None;
        }
        let pointer = scene.interact_pointer_pos()?;
        if !scene.dragged_by(voltra_core::egui::PointerButton::Primary) {
            return None;
        }
        let at = crate::picking::world_at(scene, pointer, frame);
        let subtract = ui.input(|i| i.modifiers.shift);
        let dt = ui.input(|i| i.stable_dt).min(0.1);
        let amount = self.strength * dt * if subtract { -1.0 } else { 1.0 };
        let (bone, radius) = (self.bone, self.radius);
        let world = &mut *frame.world;
        let skinned = world.get::<SkinnedSprite>(entity)?.clone();
        // Where each vertex is drawn now, and the brush taken into the
        // mesh's own space by the nearest one — so it paints what is under
        // it whatever the pose.
        let posed = pose::skin_pose(world, entity, &skinned, &|e| world_matrix_2d(world, e));
        let drawn = pose::deformed(&skinned, &posed);
        let under: Vec<usize> = drawn
            .iter()
            .enumerate()
            .filter(|(_, p)| p.distance(at) < radius)
            .map(|(i, _)| i)
            .collect();
        let component = world.get_mut::<SkinnedSprite>(entity)?;
        for i in under {
            // Softly: by the vertex's distance from the brush as drawn.
            let t = (1.0 - drawn[i].distance(at) / radius).max(0.0);
            weights::paint_vertex(
                &mut component.mesh,
                i,
                bone,
                amount * t * t * (3.0 - 2.0 * t),
            );
        }
        Some(LABEL)
    }

    /// Pushes a dot per vertex of `entity`'s mesh, coloured by how much the
    /// painted bone holds it, and the brush's circle at the pointer.
    pub fn draw(&self, scene: &Response, frame: &mut UiFrame<'_>, entity: Entity) {
        let Some(skinned) = frame.world.get::<SkinnedSprite>(entity).cloned() else {
            return;
        };
        let viewport = Vec2::new(scene.rect.width(), scene.rect.height());
        let per_pixel = frame.camera.half_extents().x * 2.0 / viewport.x.max(1.0);
        let brush = scene
            .hover_pos()
            .map(|p| crate::picking::world_at(scene, p, frame));
        let world = &*frame.world;
        let posed = pose::skin_pose(world, entity, &skinned, &|e| world_matrix_2d(world, e));
        let drawn = pose::deformed(&skinned, &posed);
        let held = weights::of_bone(&skinned.mesh, self.bone);
        let lines = frame.lines();
        let r = 2.5 * per_pixel;
        for (at, w) in drawn.iter().zip(held) {
            let color = [w, 0.2, 1.0 - w, 1.0];
            lines.push(*at - Vec2::new(r, 0.0), *at + Vec2::new(r, 0.0), 4.0, color);
        }
        if let Some(centre) = brush {
            let segments = 32;
            for i in 0..segments {
                let a = std::f32::consts::TAU * i as f32 / segments as f32;
                let b = std::f32::consts::TAU * (i + 1) as f32 / segments as f32;
                lines.push(
                    centre + Vec2::from_angle(a) * self.radius,
                    centre + Vec2::from_angle(b) * self.radius,
                    1.5,
                    [1.0, 1.0, 1.0, 0.7],
                );
            }
        }
    }
}
