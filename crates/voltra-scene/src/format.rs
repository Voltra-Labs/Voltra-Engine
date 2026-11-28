//! Reading and writing a scene as a file.

pub mod atomic;
pub mod error;
pub mod load;
pub mod migrate;
pub mod record;
pub mod refs;
pub mod registry;
pub mod save;
pub mod serial_tree;
pub mod texture_paths;

pub use error::SceneError;
pub use load::{from_scene_file, load, load_versioned, spawn_records};
pub use migrate::{parse, parse_versioned};
pub use record::{apply_record, entity_with_id, record_entity, record_scene_id};
pub use refs::SceneRefs;
pub use registry::{
    ComponentInfo, ComponentRegistry, Registration, ReplicateError, ReplicatedComponent,
};
pub use save::{save, to_scene_file, to_text, write, EntityRecord, SceneFile, VERSION};
pub use serial_tree::{SerialTree, Step, TreePath};
pub use texture_paths::{surface_texture_paths, texture_paths};
