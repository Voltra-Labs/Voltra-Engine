//! Tabs: the page a container shows, the page a tab button names, and
//! whether an element is hidden or keeps its own style.

use voltra_core::egui::{RichText, TextEdit, Ui};
use voltra_ecs::{Entity, World};
use voltra_scene::{UiHidden, UiOwnStyle, UiTab, UiTabs};

use super::active;

pub(super) fn show(ui: &mut Ui, world: &mut World, entity: Entity) -> Option<&'static str> {
    let mut claim = None;
    if let Some(tabs) = world.get_mut::<UiTabs>(entity) {
        ui.separator();
        ui.label(RichText::new("Tabs").strong())
            .on_hover_text("shows the page its tabs name, by name; hides the rest");
        ui.horizontal(|ui| {
            ui.label("showing");
            claim = claim.or(active(
                &ui.add(TextEdit::singleline(&mut tabs.current).hint_text("first tab's page")),
                "Edit tabs",
            ));
        });
    }
    if let Some(tab) = world.get_mut::<UiTab>(entity) {
        ui.separator();
        ui.label(RichText::new("Tab").strong())
            .on_hover_text("a click shows the page of this name in its Tabs container");
        ui.horizontal(|ui| {
            ui.label("page");
            claim = claim.or(active(
                &ui.add(TextEdit::singleline(&mut tab.page).hint_text("page name")),
                "Edit tab",
            ));
        });
    }
    let mut hidden = world.get::<UiHidden>(entity).is_some();
    let mut own = world.get::<UiOwnStyle>(entity).is_some();
    ui.horizontal(|ui| {
        if ui
            .checkbox(&mut hidden, "hidden")
            .on_hover_text("off the screen, with everything under it")
            .changed()
        {
            claim = claim.or(Some("Hide element"));
        }
        if ui
            .checkbox(&mut own, "own style")
            .on_hover_text("a UI theme leaves it as it is")
            .changed()
        {
            claim = claim.or(Some("Keep own style"));
        }
    });
    if hidden != world.get::<UiHidden>(entity).is_some() {
        voltra_scene::ui::hidden::set_hidden(world, entity, hidden);
    }
    if own != world.get::<UiOwnStyle>(entity).is_some() {
        if own {
            world.insert(entity, UiOwnStyle);
        } else {
            world.remove::<UiOwnStyle>(entity);
        }
    }
    claim
}
