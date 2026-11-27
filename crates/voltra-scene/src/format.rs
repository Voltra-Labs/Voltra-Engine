//! Reading and writing a scene as a file.

pub mod atomic;
pub mod error;
pub mod load;
pub mod record;
pub mod refs;
pub mod registry;
pub mod save;
pub mod serial_tree;
pub mod texture_paths;

pub use error::SceneError;
pub use load::{from_scene_file, load};
pub use record::{apply_record, entity_with_id, record_entity, record_scene_id};
pub use refs::SceneRefs;
pub use registry::{
    ComponentInfo, ComponentRegistry, Registration, ReplicateError, ReplicatedComponent,
};
pub use save::{save, to_scene_file, write, EntityRecord, SceneFile, VERSION};
pub use serial_tree::{SerialTree, Step, TreePath};
pub use texture_paths::texture_paths;
