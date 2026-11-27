//! Remote entities drawn at the interpolation clock's moment, between the
//! two samples around it.

use std::time::Duration;

use voltra_ecs::World;
use voltra_scene::ComponentRegistry;

use super::samples::Pick;
use super::ReplicationClient;

impl ReplicationClient {
    /// Draws every remote entity's interpolated components at the
    /// timeline's moment, which this moves on by `delta`.
    pub fn interpolate(
        &mut self,
        world: &mut World,
        registry: &ComponentRegistry,
        now: Duration,
        delta: f32,
    ) {
        let Some(schema) = self.schema.as_ref() else {
            return;
        };
        let Some(drawn) = self.timeline.advance(now, delta) else {
            return;
        };
        for (id, bound) in &mut self.bound {
            if self.predicted == Some(*id) || !world.is_alive(bound.entity) {
                continue;
            }
            for (index, samples) in bound.samples.iter_mut().enumerate() {
                if samples.is_empty() {
                    continue;
                }
                let Some(Some(component)) = schema.local.get(index).copied() else {
                    continue;
                };
                let value = match samples.at(drawn) {
                    Some(Pick::Hold(value)) => Ok(value.to_vec()),
                    Some(Pick::Between(from, to, t)) => {
                        match registry.lerp_replicated(component, from, to, t) {
                            Some(result) => result,
                            None => Ok(to.to_vec()),
                        }
                    }
                    None => continue,
                };
                let applied = value.and_then(|bytes| {
                    registry.apply_replicated(component, world, bound.entity, &bytes)
                });
                if let Err(error) = applied {
                    log::warn!("{}: {error}", component.name);
                }
            }
        }
    }
}
