//! The images a rich text's `[icon=…]` tags name, resolved to a texture and
//! the part of it to draw.

use voltra_assets::{AssetPath, Atlases, Handle, Textures};
use voltra_ecs::World;
use voltra_render::wgpu::{Device, Queue};
use voltra_render::Texture;

use super::markup;
use super::Text;
use crate::sprite::quad::WHOLE_UVS;

/// One `[icon=…]` of a text, as drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct TextIcon {
    /// What the tag said: `atlas.atlas.ron/3`, or an image path.
    pub reference: String,
    /// The texture it samples; `None` when the reference did not resolve —
    /// logged — and it takes its room in the line but draws nothing.
    pub texture: Option<Handle<Texture>>,
    /// Texture coordinates, top-left, bottom-left, bottom-right, top-right.
    pub uvs: [[f32; 2]; 4],
    /// Width over height of the picture, which keeps its shape in the em
    /// box the line gives it.
    pub aspect: f32,
}

impl TextIcon {
    fn unresolved(reference: &str) -> Self {
        Self {
            reference: reference.to_owned(),
            texture: None,
            uvs: WHOLE_UVS,
            aspect: 1.0,
        }
    }
}

/// Resolves every text's icons that changed since the last call: loads the
/// atlases and textures they name. Once per frame before the draw — a
/// language switch can bring a string with an icon — and cheap when
/// nothing changed (the content is only scanned for `[icon=`).
pub fn resolve_world(
    world: &mut World,
    atlases: &mut Atlases,
    textures: &mut Textures,
    device: &Device,
    queue: &Queue,
) {
    for (_, text) in world.query_mut::<Text>() {
        if !text.rich || !text.content.contains("[icon=") {
            if !text.icons.is_empty() {
                text.icons.clear();
            }
            continue;
        }
        let wanted: Vec<String> = markup::parse(&text.content)
            .runs
            .into_iter()
            .filter_map(|run| run.style.icon)
            .collect();
        let same = wanted.len() == text.icons.len()
            && wanted
                .iter()
                .zip(&text.icons)
                .all(|(w, icon)| *w == icon.reference);
        if same {
            continue;
        }
        text.icons = wanted
            .iter()
            .map(|reference| resolve(reference, atlases, textures, device, queue))
            .collect();
    }
}

/// `path.atlas.ron/N` — frame `N` of the atlas, from the texture the atlas
/// names — or an image path, all of it.
fn resolve(
    reference: &str,
    atlases: &mut Atlases,
    textures: &mut Textures,
    device: &Device,
    queue: &Queue,
) -> TextIcon {
    let framed = reference
        .rsplit_once('/')
        .and_then(|(atlas, frame)| Some((atlas, frame.parse::<u32>().ok()?)))
        .filter(|(atlas, _)| atlas.ends_with(".atlas.ron"));
    if let Some((atlas_path, index)) = framed {
        let Ok(path) = AssetPath::new(atlas_path) else {
            log::warn!("icon {reference:?}: {atlas_path:?} is not an asset path");
            return TextIcon::unresolved(reference);
        };
        let handle = atlases.load(&path);
        let atlas = atlases.get(handle);
        let (Some(frame), Some(texture)) = (atlas.frame(index), atlas.texture().cloned()) else {
            log::warn!(
                "icon {reference:?}: the atlas has no frame {index}, or names no texture to draw it from"
            );
            return TextIcon::unresolved(reference);
        };
        let Some((min, max)) = frame.uv(atlas.sheet()) else {
            log::warn!("icon {reference:?}: frame {index} is empty");
            return TextIcon::unresolved(reference);
        };
        let aspect = frame.width as f32 / frame.height.max(1) as f32;
        return TextIcon {
            reference: reference.to_owned(),
            texture: Some(textures.load(device, queue, &texture)),
            uvs: [
                [min[0], min[1]],
                [min[0], max[1]],
                [max[0], max[1]],
                [max[0], min[1]],
            ],
            aspect,
        };
    }
    let Ok(path) = AssetPath::new(reference) else {
        log::warn!("icon {reference:?} is neither an atlas frame nor an asset path");
        return TextIcon::unresolved(reference);
    };
    let texture = textures.load(device, queue, &path);
    let aspect = textures
        .try_get(texture)
        .map_or(1.0, |t| t.width() as f32 / t.height().max(1) as f32);
    TextIcon {
        reference: reference.to_owned(),
        texture: Some(texture),
        uvs: WHOLE_UVS,
        aspect,
    }
}
