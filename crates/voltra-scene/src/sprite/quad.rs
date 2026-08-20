//! Where a sprite's corners land and which texels they sample.
//!
//! One function, called by both the batch and the pick. They must agree: if
//! they ever disagree, a click lands somewhere other than the pixels it
//! appears to land on, and nothing reports it. A shared constant was enough
//! while every sprite was a unit square; with a frame's size and a
//! pixels-per-unit deciding the extents, it is not.

use voltra_assets::{Frame, Handle};
use voltra_render::glam::{UVec2, Vec2};
use voltra_render::Texture;

use super::sheets::Sheets;
use super::Sprite;

/// The whole of a texture, as every sprite sampled before frames existed.
const WHOLE: ([f32; 2], [f32; 2]) = ([0.0, 0.0], [1.0, 1.0]);

/// One sprite's quad, before its transform: how big, what to sample, what to
/// bind.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quad {
    /// Half the width and height, in world units.
    pub half_extents: Vec2,
    /// The UV for each corner [`corners`](Self::corners) returns, in order.
    pub uvs: [[f32; 2]; 4],
    /// What to bind: the sprite's texture, or the placeholder when it asks for
    /// a frame that is not there.
    pub texture: Option<Handle<Texture>>,
}

impl Quad {
    /// The corners, centred on the origin so scale and rotation act about the
    /// sprite's middle rather than dragging it away from its own position.
    ///
    /// Top-left, bottom-left, bottom-right, top-right — the order the UVs are
    /// in, with V running down because image rows do while clip space goes up.
    pub fn corners(&self) -> [Vec2; 4] {
        let Vec2 { x, y } = self.half_extents;
        [
            Vec2::new(-x, y),
            Vec2::new(-x, -y),
            Vec2::new(x, -y),
            Vec2::new(x, y),
        ]
    }

    /// Whether `local` — a point already in the sprite's own space — is on the
    /// quad.
    pub fn contains(&self, local: Vec2) -> bool {
        local.x.abs() <= self.half_extents.x && local.y.abs() <= self.half_extents.y
    }
}

/// The quad `sprite` covers, resolved against the loaded sheets.
pub fn quad(sprite: &Sprite, sheets: Sheets<'_>) -> Quad {
    let (size, uv, texture) = match slice(sprite, sheets) {
        Slice::Whole => (
            sheets.size(sprite.texture_handle),
            WHOLE,
            sprite.texture_handle,
        ),
        Slice::Cut { size, uv } => (Some(size), uv, sprite.texture_handle),
        // The checker, which is what a texture that would not load already
        // draws. Drawing nothing would be a silent failure, and drawing the
        // nearest frame would hide the mistake where it would be seen.
        Slice::Missing => (None, WHOLE, sheets.placeholder().or(sprite.texture_handle)),
    };

    let (min, max) = uv;
    let (u0, u1) = flipped(sprite.flip_x, min[0], max[0]);
    let (v0, v1) = flipped(sprite.flip_y, min[1], max[1]);

    Quad {
        half_extents: half_extents(sprite.pixels_per_unit, size),
        uvs: [[u0, v0], [u0, v1], [u1, v1], [u1, v0]],
        texture,
    }
}

/// What a sprite draws of its texture.
enum Slice {
    /// All of it.
    Whole,
    /// A rectangle of it, `size` texels across, sampled over `uv`.
    Cut {
        size: UVec2,
        uv: ([f32; 2], [f32; 2]),
    },
    /// A frame the sprite named and the sheets do not have.
    Missing,
}

fn slice(sprite: &Sprite, sheets: Sheets<'_>) -> Slice {
    // The atlas wins over `region`: a sprite that names one is asking for its
    // frames, and a leftover region under it is not a second opinion.
    if sprite.atlas.is_some() {
        let Some(atlas) = sheets.atlas(sprite.atlas_handle) else {
            return Slice::Missing;
        };
        let Some(frame) = atlas.frame(sprite.frame) else {
            return Slice::Missing;
        };
        return cut(frame, atlas.sheet());
    }

    match (sprite.region, sheets.size(sprite.texture_handle)) {
        (Some(region), Some(sheet)) => cut(region, sheet),
        // A region with no sheet to resolve it against is not a missing frame:
        // a caller without a texture store — every headless test of this
        // geometry — still draws what it drew before regions existed.
        _ => Slice::Whole,
    }
}

/// `frame` of a `sheet`-sized image, or [`Slice::Missing`] when it covers no
/// texels: a zero-area quad is not a picture of anything, and dividing by a
/// zero sheet is how a `NaN` reaches the vertex buffer.
fn cut(frame: Frame, sheet: UVec2) -> Slice {
    match frame.uv(sheet) {
        Some(uv) => Slice::Cut {
            size: frame.size(),
            uv,
        },
        None => Slice::Missing,
    }
}

/// Half the quad, in world units.
///
/// Without a pixels-per-unit the quad is the unit square and the transform's
/// scale is its size, which is the rule every scene on disk was authored
/// against. A rate that is zero, negative or not finite falls back to it as
/// well, rather than sending an infinity to the vertex buffer.
fn half_extents(pixels_per_unit: Option<f32>, size: Option<UVec2>) -> Vec2 {
    let (Some(rate), Some(size)) = (pixels_per_unit, size) else {
        return Vec2::splat(Sprite::HALF_EXTENT);
    };
    if !rate.is_finite() || rate <= 0.0 {
        return Vec2::splat(Sprite::HALF_EXTENT);
    }
    size.as_vec2() / rate * Sprite::HALF_EXTENT
}

/// The pair, swapped when the sprite is mirrored on that axis.
///
/// The UVs move and the corners do not: mirroring must not move the sprite,
/// its collider or its gizmo.
fn flipped(flip: bool, min: f32, max: f32) -> (f32, f32) {
    if flip {
        (max, min)
    } else {
        (min, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voltra_assets::{AssetPath, Atlases};
    use voltra_testkit::scratch_root;

    /// A texture store that answers for any handle.
    ///
    /// Forged handles: nothing here resolves one, it only compares them. That
    /// is exactly what [`TextureSizes`](voltra_assets::TextureSizes) exists
    /// for — the geometry has to be assertable without a GPU.
    struct Sizes {
        size: UVec2,
    }

    impl Sizes {
        fn texture() -> Handle<Texture> {
            Handle::forge(7, 0)
        }

        fn placeholder() -> Handle<Texture> {
            Handle::forge(0, 0)
        }

        fn wide() -> Self {
            Self {
                size: UVec2::new(64, 16),
            }
        }
    }

    impl voltra_assets::TextureSizes for Sizes {
        fn size(&self, _texture: Handle<Texture>) -> Option<UVec2> {
            Some(self.size)
        }

        fn placeholder(&self) -> Handle<Texture> {
            Self::placeholder()
        }
    }

    /// Four 16×16 frames in a row, as `coin.atlas.ron` on disk.
    fn atlases() -> (Atlases, AssetPath) {
        let root = scratch_root();
        std::fs::write(
            root.join("coin.atlas.ron"),
            "(version: 1, grid: Some((cell: (16, 16), columns: 4, rows: 1)))",
        )
        .expect("the fixture writes");
        let atlases = Atlases::new(&root);
        (atlases, AssetPath::new("coin.atlas.ron").expect("valid"))
    }

    fn sprite_on(atlases: &mut Atlases, path: &AssetPath, frame: u32) -> Sprite {
        let mut sprite = Sprite::default();
        sprite.set_atlas(Some(path.clone()), atlases);
        sprite.frame = frame;
        sprite
    }

    #[test]
    fn a_sprite_with_no_sheet_is_the_unit_quad_it_always_was() {
        let quad = quad(&Sprite::default(), Sheets::default());

        assert_eq!(quad.half_extents, Vec2::splat(0.5));
        assert_eq!(quad.uvs, [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]]);
        assert_eq!(quad.texture, None);
    }

    #[test]
    fn a_region_becomes_the_uvs_it_covers_on_a_non_square_sheet() {
        let sizes = Sizes::wide();
        let sprite = Sprite {
            texture_handle: Some(Sizes::texture()),
            region: Some(Frame::new(16, 0, 16, 16)),
            ..Default::default()
        };

        let quad = quad(
            &sprite,
            Sheets {
                atlases: None,
                textures: Some(&sizes),
            },
        );

        assert_eq!(
            quad.uvs,
            [[0.25, 0.0], [0.25, 1.0], [0.5, 1.0], [0.5, 0.0]],
            "a 16-texel cell of a 64x16 sheet is a quarter across and all of it down"
        );
    }

    #[test]
    fn the_frame_of_an_atlas_decides_the_uvs() {
        let (mut atlases, path) = atlases();
        let sprite = sprite_on(&mut atlases, &path, 2);

        let quad = quad(
            &sprite,
            Sheets {
                atlases: Some(&atlases),
                textures: None,
            },
        );

        assert_eq!(quad.uvs[0], [0.5, 0.0], "the third of four cells");
        assert_eq!(quad.uvs[2], [0.75, 1.0]);
    }

    #[test]
    fn a_flip_swaps_the_uvs_and_leaves_the_corners_where_they_were() {
        let (mut atlases, path) = atlases();
        let mut sprite = sprite_on(&mut atlases, &path, 0);
        let sheets = Sheets {
            atlases: Some(&atlases),
            textures: None,
        };
        let upright = quad(&sprite, sheets);

        sprite.flip_x = true;
        let mirrored = quad(&sprite, sheets);

        assert_eq!(
            mirrored.corners(),
            upright.corners(),
            "mirroring must not move the sprite, its collider or its gizmo"
        );
        assert_eq!(mirrored.uvs[0][0], upright.uvs[2][0]);
        assert_eq!(mirrored.uvs[2][0], upright.uvs[0][0]);
        assert_eq!(
            mirrored.uvs[0][1], upright.uvs[0][1],
            "and leaves the other axis alone"
        );
    }

    #[test]
    fn pixels_per_unit_sizes_the_quad_from_the_frame() {
        let sizes = Sizes::wide();
        let sprite = Sprite {
            texture_handle: Some(Sizes::texture()),
            region: Some(Frame::new(0, 0, 32, 16)),
            pixels_per_unit: Some(16.0),
            ..Default::default()
        };

        let quad = quad(
            &sprite,
            Sheets {
                atlases: None,
                textures: Some(&sizes),
            },
        );

        assert_eq!(
            quad.half_extents,
            Vec2::new(1.0, 0.5),
            "32x16 texels at 16 per unit is two units by one"
        );
    }

    #[test]
    fn a_rate_of_zero_keeps_the_unit_quad_rather_than_an_infinity() {
        let (mut atlases, path) = atlases();
        let mut sprite = sprite_on(&mut atlases, &path, 0);
        sprite.pixels_per_unit = Some(0.0);

        let quad = quad(
            &sprite,
            Sheets {
                atlases: Some(&atlases),
                textures: None,
            },
        );

        assert_eq!(quad.half_extents, Vec2::splat(0.5));
    }

    #[test]
    fn a_frame_past_the_end_draws_the_placeholder() {
        let (mut atlases, path) = atlases();
        let sizes = Sizes::wide();
        let mut sprite = sprite_on(&mut atlases, &path, 9);
        sprite.texture_handle = Some(Sizes::texture());

        let quad = quad(
            &sprite,
            Sheets {
                atlases: Some(&atlases),
                textures: Some(&sizes),
            },
        );

        assert_eq!(
            quad.texture,
            Some(Sizes::placeholder()),
            "an index past the end is an authoring mistake and must be seen"
        );
        assert_eq!(quad.uvs[2], [1.0, 1.0], "the whole checker, not a frame");
    }

    #[test]
    fn an_atlas_that_never_resolved_draws_the_placeholder_too() {
        let sizes = Sizes::wide();
        let sprite = Sprite {
            atlas: Some(AssetPath::new("gone.atlas.ron").expect("valid")),
            texture_handle: Some(Sizes::texture()),
            ..Default::default()
        };

        let quad = quad(
            &sprite,
            Sheets {
                atlases: None,
                textures: Some(&sizes),
            },
        );

        assert_eq!(quad.texture, Some(Sizes::placeholder()));
    }
}
