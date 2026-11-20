//! Shelf packing: rectangles into rows of a fixed-size sheet.
//!
//! The simplest packer that suits glyphs. Glyphs of one size are close to one
//! height, so rows ("shelves") of similar height waste little; a skyline or
//! guillotine packer would pack tighter for mixed shapes and is not worth its
//! code until an atlas shows that waste.

/// One row of the sheet: everything on it sits at `y` and is at most
/// `height` tall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shelf {
    y: u32,
    height: u32,
    /// Where the next rectangle on this shelf starts.
    cursor: u32,
}

/// Places rectangles into a `width` × `height` sheet, row by row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shelves {
    width: u32,
    height: u32,
    shelves: Vec<Shelf>,
}

impl Shelves {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            shelves: Vec::new(),
        }
    }

    /// The top-left corner a `w` × `h` rectangle was placed at, or `None`
    /// when the sheet has no room left for it.
    ///
    /// Best fit by height among the shelves with room across: the shortest
    /// shelf that holds the rectangle, so a tall glyph does not open a row a
    /// line of short ones then waste the top of. A new shelf opens below the
    /// last when none fits. An empty rectangle takes no room and is placed at
    /// the origin.
    pub fn insert(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        if w == 0 || h == 0 {
            return Some((0, 0));
        }
        if w > self.width || h > self.height {
            return None;
        }

        let best = self
            .shelves
            .iter_mut()
            .filter(|shelf| shelf.height >= h && self.width - shelf.cursor >= w)
            .min_by_key(|shelf| shelf.height);
        if let Some(shelf) = best {
            let at = (shelf.cursor, shelf.y);
            shelf.cursor += w;
            return Some(at);
        }

        let y = self.shelves.last().map_or(0, |last| last.y + last.height);
        if self.height - y < h {
            return None;
        }
        self.shelves.push(Shelf {
            y,
            height: h,
            cursor: w,
        });
        Some((0, y))
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_rectangle_lands_at_the_origin() {
        let mut shelves = Shelves::new(64, 64);
        assert_eq!(shelves.insert(10, 12), Some((0, 0)));
    }

    #[test]
    fn rectangles_fill_a_shelf_left_to_right() {
        let mut shelves = Shelves::new(64, 64);
        shelves.insert(10, 12);
        assert_eq!(shelves.insert(10, 12), Some((10, 0)));
        assert_eq!(shelves.insert(5, 8), Some((20, 0)), "shorter fits the row");
    }

    #[test]
    fn a_full_row_opens_the_next_below_it() {
        let mut shelves = Shelves::new(32, 64);
        shelves.insert(20, 12);
        assert_eq!(shelves.insert(20, 10), Some((0, 12)));
    }

    #[test]
    fn a_taller_rectangle_opens_a_new_shelf() {
        let mut shelves = Shelves::new(64, 64);
        shelves.insert(10, 8);
        assert_eq!(shelves.insert(10, 16), Some((0, 8)));
    }

    #[test]
    fn the_shortest_shelf_that_fits_is_chosen() {
        let mut packed = Shelves::new(64, 64);
        packed.insert(60, 20); // shelf A, nearly full
        packed.insert(10, 10); // opens shelf B at y 20
        assert_eq!(
            packed.insert(4, 10),
            Some((10, 20)),
            "shelf B is shorter than A and has room"
        );
    }

    #[test]
    fn a_full_sheet_refuses() {
        let mut shelves = Shelves::new(16, 16);
        assert_eq!(shelves.insert(16, 16), Some((0, 0)));
        assert_eq!(shelves.insert(1, 1), None);
    }

    #[test]
    fn a_rectangle_larger_than_the_sheet_is_refused() {
        let mut shelves = Shelves::new(16, 16);
        assert_eq!(shelves.insert(17, 1), None);
        assert_eq!(shelves.insert(1, 17), None);
    }

    #[test]
    fn an_empty_rectangle_takes_no_room() {
        let mut shelves = Shelves::new(16, 16);
        assert_eq!(shelves.insert(0, 5), Some((0, 0)));
        assert_eq!(
            shelves.insert(16, 16),
            Some((0, 0)),
            "the sheet is still empty"
        );
    }
}
