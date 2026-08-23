//! The sprite animation: its frames, its rate, and whether it is running.

use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_ecs::{Entity, World};
use voltra_scene::{Sprite, SpriteAnimation};

use super::active;

/// The clip's rows, or the button that gives the sprite one.
///
/// Absent by default, like the collision filter: a still sprite is the common
/// case, and a component every sprite carried would be in every scene file for
/// nothing.
pub(super) fn show(
    ui: &mut Ui,
    world: &mut World,
    entity: Entity,
    frames_in_atlas: usize,
) -> Option<&'static str> {
    // Tested before the mutable borrow rather than with a `let ... else`: the
    // borrow from `get_mut` outlives the `else` branch as far as the borrow
    // checker is concerned, and that branch needs the world again.
    if world.get::<SpriteAnimation>(entity).is_none() {
        return add_button(ui, world, entity);
    }
    let animation = world
        .get_mut::<SpriteAnimation>(entity)
        .expect("the component is there: it was just tested for");

    ui.label(RichText::new("Animation").strong());

    let mut claim = None;
    egui::Grid::new("animation").num_columns(2).show(ui, |ui| {
        ui.label("fps");
        let response = ui.add(
            DragValue::new(&mut animation.fps)
                .range(0.0..=240.0)
                .speed(0.5),
        );
        claim = claim.or(active(&response, "Set frame rate"));
        ui.end_row();

        ui.label("looping");
        if ui.checkbox(&mut animation.looping, "").changed() {
            claim = claim.or(Some("Loop animation"));
        }
        ui.end_row();

        ui.label("playing");
        ui.horizontal(|ui| {
            if ui.checkbox(&mut animation.playing, "").changed() {
                claim = claim.or(Some("Play animation"));
            }
            // A clip that is not looping switches itself off on its last
            // frame, so without this the only way to see it a second time
            // would be to reload the scene.
            if ui.button("Restart").clicked() {
                animation.restart();
                claim = claim.or(Some("Restart animation"));
            }
        });
        ui.end_row();
    });

    claim.or(frames_ui(ui, animation, frames_in_atlas))
}

/// The list of atlas indices, in play order.
///
/// A row of numbers rather than a timeline: the clip is a list of indices at
/// one rate, and a timeline promises per-frame durations and tracks that this
/// component does not have.
fn frames_ui(
    ui: &mut Ui,
    animation: &mut SpriteAnimation,
    frames_in_atlas: usize,
) -> Option<&'static str> {
    let mut claim = None;
    // Unbounded without a sheet to bound against: an atlas the editor has not
    // loaded yet must not silently rewrite a clip's indices to zero.
    let last = frames_in_atlas.saturating_sub(1) as u32;

    ui.horizontal_wrapped(|ui| {
        ui.label("frames");
        let mut remove = None;
        for (slot, frame) in animation.frames.iter_mut().enumerate() {
            let mut value = DragValue::new(frame);
            if frames_in_atlas > 0 {
                value = value.range(0..=last);
            }
            // Salted by the slot: without it every index in the row shares one
            // id, and typing into the third would edit the first.
            let response = ui.push_id(slot, |ui| ui.add(value)).inner;
            claim = claim.or(active(&response, "Edit frames"));
            if response.secondary_clicked() {
                remove = Some(slot);
            }
        }

        if ui.button("+").clicked() {
            // The frame after the last one, wrapped, which is what appending to
            // a run cycle almost always means.
            let next = animation
                .frames
                .last()
                .map(|frame| if frame >= &last { 0 } else { frame + 1 })
                .unwrap_or(0);
            animation.frames.push(next);
            claim = claim.or(Some("Edit frames"));
        }

        if let Some(slot) = remove {
            animation.frames.remove(slot);
            claim = claim.or(Some("Edit frames"));
        }
    });
    ui.label(
        RichText::new("right-click a number to drop that frame")
            .weak()
            .small(),
    );

    claim
}

/// The offer to animate a sprite that has no clip yet.
fn add_button(ui: &mut Ui, world: &mut World, entity: Entity) -> Option<&'static str> {
    // Only for something that can show a frame. An animation on an entity with
    // no sprite has nothing to write to.
    world.get::<Sprite>(entity)?;

    ui.button("Add animation").clicked().then(|| {
        world.insert(entity, SpriteAnimation::default());
        "Add animation"
    })
}
