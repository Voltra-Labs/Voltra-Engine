//! A composed surface shader, checked: what a surface pipeline is built
//! from.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, LazyLock};

use wgpu::naga;

use super::compose::standard_source;
use super::layouts::{CUSTOM_BLOCK_BINDING, CUSTOM_TEXTURE_BINDING, SURFACE_TEXTURES};

/// A [`compose_surface`](super::compose_surface)d shader that parsed and
/// validated, its bindings the engine's: safe to build a pipeline from.
///
/// Checked before compiled, as a sprite material is, because `wgpu`
/// reports a broken shader to an error scope rather than to its caller,
/// and a pipeline whose shader binds what its layout does not is a device
/// error; a broken material must fall back, not take the device down.
///
/// Its **revision** is its source's hash: the same text is one pipeline,
/// whichever material or preview composed it, and an edit is a new one.
#[derive(Debug)]
pub struct SurfaceShader {
    revision: u64,
    source: String,
}

static STANDARD: LazyLock<Arc<SurfaceShader>> = LazyLock::new(|| {
    Arc::new(
        SurfaceShader::new(standard_source(), 0)
            .unwrap_or_else(|e| panic!("the engine's standard surface validates: {e}")),
    )
});

impl SurfaceShader {
    /// `source`, composed with `textures` named textures, checked: it
    /// parses, validates, and binds nothing but what the composer declared
    /// — a material's own `@group` / `@binding` is refused, since the
    /// pipeline's layout is the engine's. `Err` says what and where.
    pub fn new(source: String, textures: usize) -> Result<Self, String> {
        let module =
            naga::front::wgsl::parse_str(&source).map_err(|e| e.emit_to_string(&source))?;
        check_bindings(&module, textures)?;
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .map_err(|e| e.emit_to_string(&source))?;
        let mut hasher = DefaultHasher::new();
        source.hash(&mut hasher);
        Ok(Self {
            revision: hasher.finish(),
            source,
        })
    }

    /// The standard surface's shader: a `surface()` returning the
    /// material's `standard` block. Composed and checked once a process.
    pub fn standard() -> Arc<Self> {
        Arc::clone(&STANDARD)
    }

    /// Its source's hash: what a pipeline built from it is keyed by.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The whole WGSL.
    pub fn source(&self) -> &str {
        &self.source
    }
}

impl PartialEq for SurfaceShader {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
            || (self.revision == other.revision && self.source == other.source)
    }
}

/// Every resource `module` binds is one the composer declared, once: the
/// frame (group 0), the standard block, its sampler and five textures, the
/// material's block and `textures` of its own (group 1).
fn check_bindings(module: &naga::Module, textures: usize) -> Result<(), String> {
    let mut allowed: Vec<(u32, u32)> = vec![(0, 0)];
    allowed.extend((0..2 + SURFACE_TEXTURES).map(|b| (1, b)));
    allowed.push((1, CUSTOM_BLOCK_BINDING));
    allowed.extend((0..textures as u32).map(|i| (1, CUSTOM_TEXTURE_BINDING + i)));
    let mut seen = Vec::new();
    for (_, global) in module.global_variables.iter() {
        let Some(binding) = &global.binding else {
            continue;
        };
        let at = (binding.group, binding.binding);
        let name = global.name.as_deref().unwrap_or("?");
        if !allowed.contains(&at) || seen.contains(&at) {
            return Err(format!(
                "`{name}` is bound at @group({}) @binding({}): a surface material's bindings \
                 are the engine's — name a texture in the file's `textures`, a number in its \
                 `uniforms`",
                at.0, at.1
            ));
        }
        seen.push(at);
    }
    Ok(())
}
