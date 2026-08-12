//! Reading and writing a scene as a file.

pub mod atomic;
pub mod error;
pub mod load;
pub mod record;
pub mod registry;
pub mod save;

pub use error::SceneError;
pub use load::{from_scene_file, load};
pub use record::{apply_record, entity_with_id, record_entity, record_scene_id};
pub use registry::ComponentRegistry;
pub use save::{save, to_scene_file, EntityRecord, SceneFile, VERSION};
