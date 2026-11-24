//! The two search boxes over the bridge: what they are set to, and what
//! they then show — by the same code the panels draw from.

use serde_json::{json, Value};
use voltra_bridge::Command;
use voltra_core::UiFrame;
use voltra_scene::edit::id_of;

use crate::editor::Editor;
use crate::search::{hierarchy, Query};

/// Applies `hierarchy_search` or `asset_search`.
pub(super) fn apply(
    editor: &mut Editor,
    frame: &mut UiFrame<'_>,
    command: &Command,
) -> Result<Value, String> {
    match command {
        Command::HierarchySearch { query } => {
            query.clone_into(&mut editor.hierarchy_search);
            let found = hierarchy::filter(frame.world, &editor.registry, &Query::parse(query));
            if !found.unknown.is_empty() {
                return Err(format!(
                    "no component is called {}",
                    found.unknown.join(", ")
                ));
            }
            let named = |entities: &mut dyn Iterator<Item = &voltra_ecs::Entity>| {
                let mut rows: Vec<(String, String)> = entities
                    .map(|&e| (hierarchy::label(frame.world, e), id_of(frame.world, e)))
                    .collect();
                rows.sort();
                rows.into_iter()
                    .map(|(name, id)| json!({ "name": name, "id": id }))
                    .collect::<Vec<_>>()
            };
            let matches = named(&mut found.matched.iter());
            let around = named(&mut found.shown.difference(&found.matched));
            Ok(json!({ "matches": matches, "parents": around }))
        }
        Command::AssetSearch { query } => {
            editor.assets.set_search(query);
            // Read afresh: what a tool searches for may be what it just wrote.
            editor.assets.refresh();
            let found = editor.assets.search_results(frame.textures.root());
            if !found.unknown.is_empty() {
                return Err(format!(
                    "no kind of asset is called {}",
                    found.unknown.join(", ")
                ));
            }
            let entries: Vec<Value> = found
                .entries
                .iter()
                .map(|entry| json!({ "path": entry.path.as_str(), "kind": format!("{:?}", entry.kind) }))
                .collect();
            Ok(json!({ "matches": entries }))
        }
        _ => Err("not a search command".into()),
    }
}
