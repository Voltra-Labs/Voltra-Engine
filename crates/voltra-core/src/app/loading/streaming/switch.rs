//! One streamed world given up for another, or for none.

use voltra_assets::AssetPath;
use voltra_scene::WorldRegions;

use super::super::unload::Unload;
use super::{Slot, StreamedWorld, Streaming};
use crate::app::spawning::prefab_store;
use crate::app::App;

impl App {
    /// Stops streaming the current world, if any — its regions going over
    /// the frames to come — and starts on `next`, if it reads.
    pub(super) fn switch_world(&mut self, streaming: &mut Streaming, next: Option<AssetPath>) {
        if let Some(old) = streaming.world.take() {
            // On a replication client, what was swept of the regions going.
            let mut going = Vec::new();
            for slot in old.slots {
                match slot {
                    Slot::Loading(build) => {
                        let entities = build.entities().to_vec();
                        going.extend_from_slice(build.replicated_ids());
                        streaming.orphans.push(Unload::new(entities));
                    }
                    Slot::Loaded(entities, ids) => {
                        going.extend(ids);
                        streaming.orphans.push(Unload::new(entities));
                    }
                    Slot::Unloading(unload) => streaming.orphans.push(unload),
                    Slot::Unloaded | Slot::Broken => {}
                }
            }
            if !going.is_empty() {
                self.forget_unsent_of(&going);
            }
            log::info!("stopped streaming {}", old.path.as_str());
        }
        let Some(path) = next else {
            return;
        };
        let root = prefab_store(&mut self.prefabs, &self.asset_root, &self.registry)
            .root()
            .to_path_buf();
        match WorldRegions::read(&root, &path) {
            Ok(regions) => {
                log::info!(
                    "streaming {} ({} regions)",
                    path.as_str(),
                    regions.regions.len()
                );
                let slots = regions.regions.iter().map(|_| Slot::Unloaded).collect();
                streaming.world = Some(StreamedWorld {
                    path,
                    regions,
                    slots,
                });
            }
            Err(error) => log::error!("could not stream {}: {error}", path.as_str()),
        }
    }
}
