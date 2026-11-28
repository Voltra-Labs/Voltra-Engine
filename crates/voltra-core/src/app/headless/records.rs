//! An entity's components read and written as RON in a headless run, as a
//! scene file holds them.

use std::fmt;

use ron::value::RawValue;
use voltra_scene::format::{entity_with_id, record_entity};
use voltra_scene::{SceneError, SceneId};

use super::Headless;

/// Why a headless edit could not be made.
#[derive(Debug)]
pub enum HeadlessError {
    /// No entity has this id.
    NoEntity(SceneId),
    /// No component is registered under this name.
    NoComponent(String),
    /// The value would not read as the component, or would not write.
    Scene(SceneError),
}

impl fmt::Display for HeadlessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoEntity(id) => write!(f, "no entity has the id {id:?}"),
            Self::NoComponent(name) => write!(f, "no component is registered as {name:?}"),
            Self::Scene(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for HeadlessError {}

impl From<SceneError> for HeadlessError {
    fn from(error: SceneError) -> Self {
        Self::Scene(error)
    }
}

impl Headless {
    /// The entity `id` names, as a scene file writes it — every registered
    /// component it carries, as RON.
    pub fn entity_ron(&self, id: SceneId) -> Result<String, HeadlessError> {
        let entity = entity_with_id(&self.app.world, id).ok_or(HeadlessError::NoEntity(id))?;
        let record = record_entity(&self.app.world, &self.app.registry, entity)
            .ok_or(HeadlessError::NoEntity(id))??;
        ron::ser::to_string_pretty(&record, ron::ser::PrettyConfig::default())
            .map_err(|e| HeadlessError::Scene(SceneError::Serialize(e)))
    }

    /// Sets the component registered as `name` on the entity `id` to
    /// `value`, RON as a scene file holds it — see
    /// [`ComponentRegistry::template`] for where to start.
    pub fn set_component(
        &mut self,
        id: SceneId,
        name: &str,
        value: &str,
    ) -> Result<(), HeadlessError> {
        let entity = entity_with_id(&self.app.world, id).ok_or(HeadlessError::NoEntity(id))?;
        let raw =
            RawValue::from_ron(value).map_err(|e| HeadlessError::Scene(SceneError::Parse(e)))?;
        self.app
            .registry
            .load_one(&mut self.app.world, entity, name, raw)
            .ok_or_else(|| HeadlessError::NoComponent(name.to_owned()))??;
        // A component read from text names no handles yet.
        self.app.spawned.push(entity);
        self.app.resolve_spawned();
        Ok(())
    }
}
