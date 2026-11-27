//! The animation player: which clip, whether it plays, and how fast.

use voltra_assets::AssetPath;
use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_ecs::{Entity, World};
use voltra_scene::{AnimationPlayer, Transform2D};

use super::active;
use super::path_field::{self, PathEdit};

/// The player's rows, or the button that gives the entity one.
///
/// Absent by default, like the sprite's flipbook: most entities do not
/// move on their own, and a component every one carried would be in every
/// scene file for nothing.
pub(super) fn show(ui: &mut Ui, world: &mut World, entity: Entity) -> Option<&'static str> {
    if world.get::<AnimationPlayer>(entity).is_none() {
        // Anything placed can be animated: every property moves an entity
        // with a transform, or something drawn on one.
        world.get::<Transform2D>(entity)?;
        ui.separator();
        return ui.button("Add animation player").clicked().then(|| {
            world.insert(entity, AnimationPlayer::default());
            "Add animation player"
        });
    }
    let player = world
        .get_mut::<AnimationPlayer>(entity)
        .expect("the component is there: it was just tested for");

    ui.separator();
    ui.label(RichText::new("Animation player").strong())
        .on_hover_text("plays a .anim.ron clip on this entity and the children it names");
    let mut claim = clip_ui(ui, player);

    egui::Grid::new("animation_player")
        .num_columns(2)
        .show(ui, |ui| {
            ui.label("speed")
                .on_hover_text("negative plays it backwards");
            claim = claim.or(active(
                &ui.add(
                    DragValue::new(&mut player.speed)
                        .speed(0.01)
                        .range(-10.0..=10.0),
                ),
                "Set animation speed",
            ));
            ui.end_row();

            ui.label("playing");
            ui.horizontal(|ui| {
                if ui.checkbox(&mut player.playing, "").changed() {
                    claim = claim.or(Some("Play animation"));
                }
                // A clip that does not loop turns itself off at its end.
                if ui.button("Restart").clicked() {
                    player.restart();
                    claim = claim.or(Some("Restart animation"));
                }
                ui.label(RichText::new(format!("{:.2}s", player.time())).weak());
            });
            ui.end_row();
        });
    claim
}

/// The clip's path: typed, or dropped from the browser. Only a
/// `.anim.ron` is taken — any other file would read as an empty clip and
/// the player would hold still without saying why.
fn clip_ui(ui: &mut Ui, player: &mut AnimationPlayer) -> Option<&'static str> {
    let committed = player
        .clip
        .as_ref()
        .map(AssetPath::as_str)
        .unwrap_or("")
        .to_owned();
    let path = match path_field::show(ui, "anim_clip", "path/to/clip.anim.ron", &committed)? {
        PathEdit::Cleared => None,
        PathEdit::Set(path) if voltra_assets::watch::is_animation(path.as_str()) => Some(path),
        PathEdit::Set(path) => {
            log::warn!("{} is not an animation clip (*.anim.ron)", path.as_str());
            return None;
        }
    };
    match path {
        Some(path) => player.play(path),
        None => player.clip = None,
    }
    Some("Set animation clip")
}
