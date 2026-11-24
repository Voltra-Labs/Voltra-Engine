//! Which way round a tile is: Tiled's three flags, and turning them.
//!
//! A tile is drawn mirrored by any of eight ways — the symmetries of a
//! square. Tiled stores them as three bits, applied in a fixed order: the
//! diagonal flip (a transpose, swapping the image's x and y) first, then the
//! horizontal flip, then the vertical. Godot 4's `TRANSFORM_TRANSPOSE`,
//! `FLIP_H`, `FLIP_V` are the same three. A quarter turn is a transpose and
//! a flip, so rotating a stamp needs no fourth bit; composing two
//! orientations is a product of 2×2 signed permutation matrices, read back
//! into the three flags.
//!
//! All in *image* space — `x` right, `y` down — because that is what the
//! flags mean to the sheet; the map's cells run `y` up, and a quarter turn
//! "clockwise" is clockwise as seen on screen either way.

use voltra_render::glam::Vec2;

use super::Tile;

/// An orientation as a matrix acting on image coordinates centred on the
/// tile: rows `[a, b]`, `[c, d]`, each entry -1, 0 or 1.
type Matrix = [[i8; 2]; 2];

fn matrix(tile: Tile) -> Matrix {
    // Displayed = V · H · D applied to the image, in that order.
    let mut m: Matrix = [[1, 0], [0, 1]];
    if tile.transpose {
        m = mul([[0, 1], [1, 0]], m);
    }
    if tile.flip_x {
        m = mul([[-1, 0], [0, 1]], m);
    }
    if tile.flip_y {
        m = mul([[1, 0], [0, -1]], m);
    }
    m
}

fn mul(a: Matrix, b: Matrix) -> Matrix {
    let mut out = [[0; 2]; 2];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = a[i][0] * b[0][j] + a[i][1] * b[1][j];
        }
    }
    out
}

/// `tile` with its orientation replaced by `m`'s.
fn with(tile: Tile, m: Matrix) -> Tile {
    let transpose = m[0][0] == 0;
    // Undo the transpose to read the flips off the diagonal: M = V·H·D, so
    // M·D is V·H, diagonal.
    let flips = if transpose {
        mul(m, [[0, 1], [1, 0]])
    } else {
        m
    };
    Tile {
        frame: tile.frame,
        flip_x: flips[0][0] < 0,
        flip_y: flips[1][1] < 0,
        transpose,
    }
}

impl Tile {
    /// The tile turned a quarter clockwise, as seen.
    pub fn rotated_cw(self) -> Self {
        // In image space, y down: (x, y) → (-y, x) turns clockwise on screen.
        with(self, mul([[0, -1], [1, 0]], matrix(self)))
    }

    /// The tile turned a quarter anticlockwise, as seen.
    pub fn rotated_ccw(self) -> Self {
        with(self, mul([[0, 1], [-1, 0]], matrix(self)))
    }

    /// The tile mirrored left to right, as seen — whatever it was already.
    pub fn mirrored_x(self) -> Self {
        with(self, mul([[-1, 0], [0, 1]], matrix(self)))
    }

    /// The tile mirrored top to bottom, as seen.
    pub fn mirrored_y(self) -> Self {
        with(self, mul([[1, 0], [0, -1]], matrix(self)))
    }

    /// Where the point `p` of the tile *as drawn* — `0..1` across, `0..1`
    /// down the cell — samples its frame, `0..1` across and down the frame.
    /// The inverse of the orientation; what the mesh's UVs and the tile's
    /// collision shape are both turned by.
    pub fn sample(self, p: Vec2) -> Vec2 {
        let a = if self.flip_y { 1.0 - p.y } else { p.y };
        let b = if self.flip_x { 1.0 - p.x } else { p.x };
        if self.transpose {
            Vec2::new(a, b)
        } else {
            Vec2::new(b, a)
        }
    }

    /// Where the point `q` of the frame — `0..1` across and down it — is
    /// drawn in the cell: the inverse of [`sample`](Self::sample).
    pub fn place(self, q: Vec2) -> Vec2 {
        let (b, a) = if self.transpose {
            (q.y, q.x)
        } else {
            (q.x, q.y)
        };
        let x = if self.flip_x { 1.0 - b } else { b };
        let y = if self.flip_y { 1.0 - a } else { a };
        Vec2::new(x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Tile; 8] = {
        let mut all = [Tile {
            frame: 3,
            flip_x: false,
            flip_y: false,
            transpose: false,
        }; 8];
        let mut i = 0;
        while i < 8 {
            all[i].flip_x = i & 1 != 0;
            all[i].flip_y = i & 2 != 0;
            all[i].transpose = i & 4 != 0;
            i += 1;
        }
        all
    };

    #[test]
    fn four_quarter_turns_are_none() {
        for tile in ALL {
            let turned = tile.rotated_cw().rotated_cw().rotated_cw().rotated_cw();
            assert_eq!(turned, tile);
            assert_eq!(tile.rotated_cw().rotated_ccw(), tile);
        }
    }

    #[test]
    fn a_quarter_turn_is_tileds_diagonal_and_horizontal_flags() {
        // Tiled: "rotate 90° clockwise" sets the diagonal and horizontal bits.
        let turned = Tile::new(3).rotated_cw();
        assert!(turned.transpose && turned.flip_x && !turned.flip_y);
    }

    #[test]
    fn a_quarter_turn_moves_the_top_left_to_the_top_right() {
        let turned = Tile::new(0).rotated_cw();
        // The frame's top-left texel is drawn at the cell's top-right.
        let drawn = turned.place(Vec2::new(0.0, 0.0));
        assert_eq!(drawn, Vec2::new(1.0, 0.0));
        assert_eq!(turned.sample(drawn), Vec2::ZERO);
    }

    #[test]
    fn mirroring_twice_is_nothing_and_sample_inverts_place() {
        let points = [
            Vec2::new(0.25, 0.0),
            Vec2::new(1.0, 0.75),
            Vec2::new(0.5, 0.5),
        ];
        for tile in ALL {
            assert_eq!(tile.mirrored_x().mirrored_x(), tile);
            assert_eq!(tile.mirrored_y().mirrored_y(), tile);
            for p in points {
                assert_eq!(tile.sample(tile.place(p)), p, "{tile:?}");
            }
        }
    }
}
