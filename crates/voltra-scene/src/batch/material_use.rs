//! The materials a batch draws through, each with one set of values.

use voltra_assets::{Handle, Material};
use voltra_render::material::BLOCK_SIZE;

use super::SpriteBatch;
use crate::sprite::sheets::Sheets;
use crate::SpriteMaterial;

/// One material with one set of uniform values, as a batch's ranges name
/// it: what the draw binds — the material's pipeline, made from its
/// `revision`, and the block of uniforms uploaded for this frame.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialUse {
    pub material: Handle<Material>,
    pub revision: u64,
    pub block: [u8; BLOCK_SIZE],
}

impl SpriteBatch {
    /// The index of `slot`'s material and values in
    /// [`materials`](Self::materials), added if new; `None` — the plain
    /// sprite shader — for no material, one not resolved, one `sheets` has
    /// no store for, or one that does not compile.
    pub fn material(&mut self, slot: Option<&SpriteMaterial>, sheets: Sheets<'_>) -> Option<u32> {
        let slot = slot?;
        let handle = slot.handle?;
        let material = sheets.materials?.get(handle)?;
        let layout = material.layout.as_ref().filter(|_| material.is_usable())?;
        let block = layout.block(&slot.uniforms);
        let used = MaterialUse {
            material: handle,
            revision: material.revision,
            block,
        };
        if let Some(index) = self.materials.iter().position(|m| *m == used) {
            return Some(index as u32);
        }
        self.materials.push(used);
        Some(self.materials.len() as u32 - 1)
    }
}
