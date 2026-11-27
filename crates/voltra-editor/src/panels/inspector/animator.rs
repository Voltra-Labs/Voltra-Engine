//! The animator: which state machine, where it is, its parameters, and
//! the machine's layers, transitions and blend trees.

mod edits;
mod layers;
mod transitions;
mod trees;

use voltra_assets::AssetPath;
use voltra_core::egui::{self, DragValue, RichText, Ui};
use voltra_ecs::{Entity, World};
use voltra_scene::animator::ParamValue;
use voltra_scene::{AnimationPlayer, Animator, StateMachines};

use super::path_field::{self, PathEdit};

/// The animator's rows, or the button that gives an entity with a player
/// one.
pub(super) fn show(
    ui: &mut Ui,
    world: &mut World,
    machines: &mut StateMachines,
    entity: Entity,
) -> Option<&'static str> {
    if world.get::<Animator>(entity).is_none() {
        world.get::<AnimationPlayer>(entity)?;
        return ui
            .button("Add animator")
            .on_hover_text("chooses the player's clip from a state machine and parameters")
            .clicked()
            .then(|| {
                world.insert(entity, Animator::default());
                "Add animator"
            });
    }
    // What a layer's mask can name: the entities under this one.
    let targets: Vec<String> = voltra_scene::keyframes::target::paths(world, entity)
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    let animator = world
        .get_mut::<Animator>(entity)
        .expect("the component is there: it was just tested for");
    ui.separator();
    ui.label(RichText::new(Animator::NAME).strong())
        .on_hover_text("runs a .animator.ron state machine while the game plays");
    let claim = machine_ui(ui, animator);
    if let Some(state) = animator.current_state() {
        ui.label(RichText::new(format!("in {state}")).weak());
    }

    let Some(path) = animator.machine.clone() else {
        return claim;
    };
    edits::show(ui, machines, &path, &targets);
    let handle = machines.load(&path);
    for layer in &machines.get(handle).layers {
        if let Some(state) = animator.layer_state(&layer.name) {
            ui.label(RichText::new(format!("layer {} in {state}", layer.name)).weak());
        }
    }
    let parameters = machines.get(handle).parameters.clone();
    if parameters.is_empty() {
        return claim;
    }
    ui.label(
        RichText::new("parameters — set while playing")
            .weak()
            .small(),
    );
    egui::Grid::new("animator_parameters")
        .num_columns(2)
        .show(ui, |ui| {
            for parameter in parameters {
                ui.label(&parameter.name);
                let value = animator
                    .parameter(&parameter.name)
                    .unwrap_or(parameter.value);
                match value {
                    ParamValue::Float(mut v) => {
                        if ui.add(DragValue::new(&mut v).speed(0.05)).changed() {
                            animator.set_float(&parameter.name, v);
                        }
                    }
                    ParamValue::Bool(mut b) => {
                        if ui.checkbox(&mut b, "").changed() {
                            animator.set_bool(&parameter.name, b);
                        }
                    }
                    ParamValue::Trigger(set) => {
                        let label = if set { "set" } else { "Trigger" };
                        if ui.button(label).clicked() {
                            animator.trigger(&parameter.name);
                        }
                    }
                }
                ui.end_row();
            }
        });
    claim
}

/// The machine's path: typed, or dropped from the browser. Only a
/// `.animator.ron` is taken.
fn machine_ui(ui: &mut Ui, animator: &mut Animator) -> Option<&'static str> {
    let committed = animator
        .machine
        .as_ref()
        .map(AssetPath::as_str)
        .unwrap_or("")
        .to_owned();
    let path = match path_field::show(
        ui,
        "animator_machine",
        "path/to/hero.animator.ron",
        &committed,
    )? {
        PathEdit::Cleared => None,
        PathEdit::Set(path) if voltra_assets::watch::is_animator(path.as_str()) => Some(path),
        PathEdit::Set(path) => {
            log::warn!("{} is not a state machine (*.animator.ron)", path.as_str());
            return None;
        }
    };
    animator.machine = path;
    animator.restart();
    Some("Set animator machine")
}
