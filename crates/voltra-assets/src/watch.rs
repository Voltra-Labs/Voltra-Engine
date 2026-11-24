//! Turning filesystem events into the [`AssetPath`]s that changed.
//!
//! One concept, and deliberately not two: this knows nothing about textures,
//! handles or the GPU. [`Textures::reload`](crate::textures::Textures::reload)
//! consumes what this produces, and the caller — `voltra_core::App` — is what
//! joins them. That seam is where a second kind of asset would attach.
//!
//! There is no subscriber registry, because there is one consumer and the shape
//! of the second is not known.

//!
//! The watcher itself is native: a browser has no files to watch, and its
//! [`AssetWatcher`] says so when asked to start.

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
pub use native::AssetWatcher;
#[cfg(target_arch = "wasm32")]
pub use web::AssetWatcher;

use crate::atlas::is_atlas;

/// File extensions that can become a texture, lowercase, without the dot.
///
/// The filter is by extension rather than by event kind, which is Unreal's
/// shape too. It also means the scene format's atomic save — `demo.ron.tmp`,
/// renamed over `demo.ron` — is ignored without a rule about our own temporary
/// files.
pub const TEXTURE_EXTENSIONS: &[&str] = &["png"];

/// Whether a file name ends in an extension this engine can load as a texture.
///
/// Here rather than beside the browser because this is where the list lives,
/// and a second copy of the match would drift from it.
pub fn is_texture(name: &str) -> bool {
    has_extension(name, TEXTURE_EXTENSIONS)
}

/// File extensions that can become a [`Clip`](voltra_audio::Clip), lowercase,
/// without the dot.
///
/// Exactly the formats the workspace builds `symphonia` with. A name here
/// whose decoder is not compiled in would be watched, listed as a sound, and
/// then fail to decode every time.
pub const CLIP_EXTENSIONS: &[&str] = &["wav", "ogg"];

/// Whether a file name is something [`Clips`](crate::Clips) loads: a sound
/// in one of [`CLIP_EXTENSIONS`], or a clip's settings (`*.clip.ron`).
pub fn is_clip(name: &str) -> bool {
    has_extension(name, CLIP_EXTENSIONS) || is_clip_settings(name)
}

/// Whether a file name is a clip's settings: `*.clip.ron`, naming a sound and
/// saying how to play it — streamed, and where it loops.
pub fn is_clip_settings(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".clip.ron")
}

/// File extensions that can become a [`Font`](crate::Font), lowercase,
/// without the dot. What `fontdue` parses: TrueType and OpenType outlines.
pub const FONT_EXTENSIONS: &[&str] = &["ttf", "otf"];

/// Whether a file name ends in an extension this engine can load as a font.
pub fn is_font(name: &str) -> bool {
    has_extension(name, FONT_EXTENSIONS)
}

/// Whether a file name is an input map: `*.input.ron`, the suffix that
/// keeps it out of the scene list the way `*.atlas.ron` keeps a slicing out.
pub fn is_input_map(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".input.ron")
}

/// Whether a file name is a project's sorting layers: `*.layers.ron`.
pub fn is_sorting_layers(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".layers.ron")
}

/// Whether a file name is a property-animation clip: `*.anim.ron`.
pub fn is_animation(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".anim.ron")
}

/// Whether a file name is an animation state machine: `*.animator.ron`.
pub fn is_animator(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".animator.ron")
}

/// Whether a file name is a scene, which is also what a prefab is.
///
/// Any `.ron` that is not a slicing, an input map, a layer list, a clip
/// or a state machine. The atomic save's
/// `demo.ron.tmp` does not match, so a save is seen once, as the rename
/// lands.
pub fn is_scene(name: &str) -> bool {
    has_extension(name, &["ron"])
        && !is_atlas(name)
        && !is_clip_settings(name)
        && !is_input_map(name)
        && !is_sorting_layers(name)
        && !is_animation(name)
        && !is_animator(name)
}

fn has_extension(name: &str, extensions: &[&str]) -> bool {
    let Some((_, extension)) = name.rsplit_once('.') else {
        return false;
    };
    extensions.contains(&extension.to_ascii_lowercase().as_str())
}
