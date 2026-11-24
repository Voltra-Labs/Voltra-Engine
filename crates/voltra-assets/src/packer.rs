//! A folder of pictures packed into one sheet, and the `.atlas.ron` that cuts
//! it back into them.
//!
//! What `cargo xtask pack-atlas` runs and what the editor calls: one packer,
//! so a sheet packed from either is the same sheet. Not to be confused with
//! [`pack`](crate::pack), the *asset pack* a web build fetches.
//!
//! **Max-rects, biggest first**, as TexturePacker, libGDX's `TexturePacker`
//! and Unity's sprite packer do ([`maxrects`]). **Padding and extruded
//! edges** against bleeding: each picture is copied with its outermost
//! texels repeated outward, so a bilinear sample half a texel past a frame's
//! edge reads the edge, not a neighbour. **Optional trimming** of
//! transparent borders, with the offset recorded so the frame still draws
//! where the whole picture would have. **Stable indices**: a re-pack keeps
//! every region's index by its name ([`order`]), because a sprite names its
//! frame by index and a level must not change when a file is added.

mod folder;
mod maxrects;
mod order;
mod pixels;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::PathBuf;

use image::RgbaImage;
use voltra_render::glam::UVec2;

use crate::atlas::{Atlas, Frame, FrameMeta, Trim, ATLAS_SUFFIX};
use maxrects::MaxRects;

pub use folder::{default_atlas_path, pack_folder, read_folder, sheet_path, PackReport};
use order::Slot;

/// How a sheet is packed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackOptions {
    /// Transparent texels between two regions' extruded edges.
    pub padding: u32,
    /// How many times each picture's outermost texels are repeated outward.
    pub extrude: u32,
    /// Whether transparent borders are cut off before packing.
    pub trim: bool,
    /// The widest and tallest sheet allowed, in texels. 4096 is what every
    /// WebGL2 device is guaranteed to sample (`MAX_TEXTURE_SIZE`'s minimum
    /// there), and so what a web build can count on.
    pub max_size: u32,
}

impl Default for PackOptions {
    fn default() -> Self {
        Self {
            padding: 2,
            extrude: 1,
            trim: false,
            max_size: 4096,
        }
    }
}

/// One picture to pack, and the name its region gets.
#[derive(Debug, Clone)]
pub struct Picture {
    /// The region's name: its path under the folder, without the extension,
    /// `/`-separated — `walk/0` for `walk/0.png`.
    pub name: String,
    /// Its pixels.
    pub image: RgbaImage,
}

/// A packed sheet and its slicing.
#[derive(Debug, Clone)]
pub struct Packed {
    /// The sheet.
    pub sheet: RgbaImage,
    /// Its slicing: one named frame per picture, at the index it had last
    /// time if it had one.
    pub atlas: Atlas,
    /// Regions the previous atlas had and no picture supplies any more,
    /// whose indices were kept empty.
    pub gone: Vec<String>,
}

/// Why a sheet could not be packed.
#[derive(Debug)]
pub enum PackError {
    /// A file or folder could not be read or written.
    Io(PathBuf, std::io::Error),
    /// A picture would not decode, or the sheet would not encode.
    Image(PathBuf, image::ImageError),
    /// The folder holds no PNG.
    Empty(PathBuf),
    /// Two files give the same region name (`a.png` and `a.PNG`).
    Duplicate(String),
    /// The pictures do not fit in a sheet of the largest size allowed.
    TooBig {
        /// The largest side allowed.
        max_size: u32,
    },
    /// The atlas already at the output path will not read, so the names it
    /// would keep are unknown; it is left alone.
    Previous(PathBuf, String),
    /// The output path is not an `.atlas.ron`.
    NotAnAtlas(PathBuf),
}

impl fmt::Display for PackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(path, e) => write!(f, "{}: {e}", path.display()),
            Self::Image(path, e) => write!(f, "{}: {e}", path.display()),
            Self::Empty(folder) => write!(f, "{} holds no PNG to pack", folder.display()),
            Self::Duplicate(name) => write!(f, "two files would both be region {name:?}"),
            Self::TooBig { max_size } => write!(
                f,
                "the pictures do not fit on a {max_size}×{max_size} sheet; split the folder"
            ),
            Self::Previous(path, e) => write!(
                f,
                "{} is there and will not read ({e}); fix or delete it, the packer keeps its names",
                path.display()
            ),
            Self::NotAnAtlas(path) => {
                write!(f, "{} does not end in {ATLAS_SUFFIX}", path.display())
            }
        }
    }
}

impl std::error::Error for PackError {}

/// Packs `pictures` into one sheet. `previous`, the atlas this one replaces,
/// decides the index of every region it already had, and its pivots are kept.
pub fn pack(
    pictures: Vec<Picture>,
    previous: Option<&Atlas>,
    options: &PackOptions,
) -> Result<Packed, PackError> {
    let mut by_name: BTreeMap<String, RgbaImage> = BTreeMap::new();
    for picture in pictures {
        if by_name
            .insert(picture.name.clone(), picture.image)
            .is_some()
        {
            return Err(PackError::Duplicate(picture.name));
        }
    }
    let names: BTreeSet<String> = by_name.keys().cloned().collect();
    let before: Vec<Option<String>> = previous.map_or_else(Vec::new, |atlas| {
        (0..atlas.len() as u32)
            .map(|i| atlas.name(i).map(str::to_owned))
            .collect()
    });
    let slots = order::slots(&names, &before);

    // What of each picture goes on the sheet: all of it, or its opaque part.
    let cuts: BTreeMap<&str, (UVec2, UVec2)> = by_name
        .iter()
        .map(|(name, image)| {
            let cut = if options.trim {
                pixels::opaque_bounds(image)
            } else {
                (UVec2::ZERO, UVec2::from(image.dimensions()))
            };
            (name.as_str(), cut)
        })
        .collect();

    let grow = UVec2::splat(2 * options.extrude + options.padding);
    let mut order: Vec<(&str, UVec2)> = cuts
        .iter()
        .map(|(name, (_, size))| (*name, *size + grow))
        .collect();
    // Biggest first, by the longer side and then the area; the name last, so
    // the same folder packs the same way every time.
    order.sort_by(|a, b| {
        let key = |size: UVec2| (size.max_element(), size.x as u64 * size.y as u64);
        key(b.1).cmp(&key(a.1)).then_with(|| a.0.cmp(b.0))
    });

    let (sheet_size, placed) = place(&order, options).ok_or(PackError::TooBig {
        max_size: options.max_size,
    })?;

    let mut sheet = RgbaImage::new(sheet_size.x, sheet_size.y);
    let mut frames: BTreeMap<&str, Frame> = BTreeMap::new();
    for (name, at) in &placed {
        let (offset, size) = cuts[name];
        let to = *at + UVec2::splat(options.extrude);
        pixels::blit_extruded(
            &mut sheet,
            &by_name[*name],
            offset,
            size,
            to,
            options.extrude,
        );
        frames.insert(name, Frame::new(to.x, to.y, size.x, size.y));
    }

    let mut atlas = Atlas::new(sheet_size, Vec::new());
    let mut gone = Vec::new();
    for slot in slots {
        match slot {
            Slot::Region(name) => {
                let image = &by_name[&name];
                let full = UVec2::from(image.dimensions());
                let (offset, size) = cuts[name.as_str()];
                let trim = (offset != UVec2::ZERO || size != full).then_some(Trim {
                    offset,
                    source: full,
                });
                let pivot = previous
                    .and_then(|atlas| atlas.index_of(&name))
                    .and_then(|index| previous?.meta(index)?.pivot);
                let frame = frames[name.as_str()];
                atlas.push(
                    frame,
                    FrameMeta {
                        name: Some(name),
                        trim,
                        pivot,
                    },
                );
            }
            Slot::Gone(name) => {
                if let Some(name) = &name {
                    gone.push(name.clone());
                }
                atlas.push(
                    Frame::default(),
                    FrameMeta {
                        name,
                        ..FrameMeta::default()
                    },
                );
            }
        }
    }
    Ok(Packed { sheet, atlas, gone })
}

/// Where every rectangle of `order` goes, on the smallest sheet that holds
/// them all: power-of-two sides, the smaller area first, the squarer of two
/// equal areas first, wider before taller.
fn place<'a>(
    order: &[(&'a str, UVec2)],
    options: &PackOptions,
) -> Option<(UVec2, Vec<(&'a str, UVec2)>)> {
    let padding = options.padding;
    let needed: u64 = order.iter().map(|(_, s)| s.x as u64 * s.y as u64).sum();
    let mut sides: Vec<u32> = (0..32)
        .map(|bit| 1u32 << bit)
        .take_while(|&side| side < options.max_size)
        .collect();
    sides.push(options.max_size.max(1));
    let mut sizes: Vec<UVec2> = sides
        .iter()
        .flat_map(|&w| sides.iter().map(move |&h| UVec2::new(w, h)))
        .filter(|size| {
            (size.x as u64 + padding as u64) * (size.y as u64 + padding as u64) >= needed
        })
        .collect();
    sizes.sort_by_key(|size| {
        (
            size.x as u64 * size.y as u64,
            size.x.abs_diff(size.y),
            std::cmp::Reverse(size.x),
        )
    });

    'sizes: for size in sizes {
        // The bin is wider than the sheet by one padding: the gap after the
        // last column and row would fall off the sheet, and needs no room.
        let mut bin = MaxRects::new(size.x + padding, size.y + padding);
        let mut placed = Vec::with_capacity(order.len());
        for &(name, cell) in order {
            let Some(rect) = bin.insert(cell.x, cell.y) else {
                continue 'sizes;
            };
            placed.push((name, UVec2::new(rect.x, rect.y)));
        }
        return Some((size, placed));
    }
    None
}

#[cfg(test)]
mod tests;
