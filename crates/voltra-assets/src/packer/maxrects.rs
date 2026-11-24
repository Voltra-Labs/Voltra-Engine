//! Rectangles packed into a bin: Jylänki's max-rects, best short side fit.
//!
//! The bin keeps every maximal free rectangle — overlapping, unlike a guillotine
//! split's — and a rectangle goes where its shorter leftover side is
//! smallest. Jylänki's survey ("A Thousand Ways to Pack the Bin", 2010) found
//! this the best of the simple heuristics, and it is what TexturePacker,
//! libGDX's packer and `rectpack2D` all build on. Offline: the caller sorts
//! the rectangles biggest first, which is most of the quality.

/// A rectangle in the bin, in texels: `x`, `y` its top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    /// Left edge.
    pub x: u32,
    /// Top edge.
    pub y: u32,
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
}

impl Rect {
    fn right(&self) -> u32 {
        self.x + self.width
    }

    fn bottom(&self) -> u32 {
        self.y + self.height
    }

    fn contains(&self, other: &Rect) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
    }

    fn overlaps(&self, other: &Rect) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }
}

/// A bin being filled.
#[derive(Debug)]
pub struct MaxRects {
    free: Vec<Rect>,
}

impl MaxRects {
    /// An empty bin of `width` by `height` texels.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            free: vec![Rect {
                x: 0,
                y: 0,
                width,
                height,
            }],
        }
    }

    /// Places a `width` by `height` rectangle, or `None` when it fits nowhere.
    /// Never rotated: a sprite turned on the sheet would need every UV of it
    /// turned back, and no sprite here reads a rotation flag.
    pub fn insert(&mut self, width: u32, height: u32) -> Option<Rect> {
        let placed = self.best_fit(width, height)?;
        let mut split = Vec::new();
        self.free.retain(|free| {
            if !free.overlaps(&placed) {
                return true;
            }
            split.extend(around(free, &placed));
            false
        });
        self.free.extend(split);
        self.prune();
        Some(placed)
    }

    /// The free rectangle whose shorter leftover side is the smallest, its
    /// longer leftover side breaking ties, then its position — so the same
    /// input packs the same way on every machine.
    fn best_fit(&self, width: u32, height: u32) -> Option<Rect> {
        self.free
            .iter()
            .filter(|free| free.width >= width && free.height >= height)
            .min_by_key(|free| {
                let dx = free.width - width;
                let dy = free.height - height;
                (dx.min(dy), dx.max(dy), free.y, free.x)
            })
            .map(|free| Rect {
                x: free.x,
                y: free.y,
                width,
                height,
            })
    }

    /// Drops every free rectangle another one contains: they are not maximal,
    /// and keeping them makes every later search slower for nothing.
    fn prune(&mut self) {
        let mut keep = vec![true; self.free.len()];
        for i in 0..self.free.len() {
            for j in 0..self.free.len() {
                if i != j && keep[j] && self.free[j].contains(&self.free[i]) {
                    // Of two equal rectangles, the later one goes.
                    if self.free[i] != self.free[j] || i > j {
                        keep[i] = false;
                        break;
                    }
                }
            }
        }
        let mut index = 0;
        self.free.retain(|_| {
            index += 1;
            keep[index - 1]
        });
    }
}

/// What is left of `free` around `used`: up to four maximal rectangles, one
/// on each side the used one does not reach.
fn around(free: &Rect, used: &Rect) -> Vec<Rect> {
    let mut parts = Vec::with_capacity(4);
    if used.x > free.x {
        parts.push(Rect {
            width: used.x - free.x,
            ..*free
        });
    }
    if used.right() < free.right() {
        parts.push(Rect {
            x: used.right(),
            width: free.right() - used.right(),
            ..*free
        });
    }
    if used.y > free.y {
        parts.push(Rect {
            height: used.y - free.y,
            ..*free
        });
    }
    if used.bottom() < free.bottom() {
        parts.push(Rect {
            y: used.bottom(),
            height: free.bottom() - used.bottom(),
            ..*free
        });
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlap(placed: &[Rect]) -> bool {
        placed
            .iter()
            .enumerate()
            .any(|(i, a)| placed[i + 1..].iter().any(|b| a.overlaps(b)))
    }

    #[test]
    fn four_quarters_fill_the_bin_exactly() {
        let mut bin = MaxRects::new(32, 32);
        let placed: Vec<Rect> = (0..4)
            .map(|_| bin.insert(16, 16).expect("a quarter fits"))
            .collect();

        assert!(!overlap(&placed));
        assert_eq!(bin.insert(1, 1), None, "and leave nothing");
    }

    #[test]
    fn mixed_sizes_never_overlap_and_stay_inside() {
        let mut bin = MaxRects::new(64, 64);
        let sizes = [
            (30, 10),
            (10, 30),
            (20, 20),
            (5, 40),
            (40, 5),
            (12, 12),
            (8, 8),
        ];
        let placed: Vec<Rect> = sizes
            .iter()
            .map(|&(w, h)| bin.insert(w, h).expect("room for all"))
            .collect();

        assert!(!overlap(&placed));
        let whole = Rect {
            x: 0,
            y: 0,
            width: 64,
            height: 64,
        };
        assert!(placed.iter().all(|rect| whole.contains(rect)));
    }

    #[test]
    fn a_rectangle_bigger_than_the_bin_fits_nowhere() {
        let mut bin = MaxRects::new(16, 16);
        assert_eq!(bin.insert(17, 1), None);
        assert_eq!(bin.insert(0, 0).map(|r| (r.x, r.y)), Some((0, 0)));
    }
}
