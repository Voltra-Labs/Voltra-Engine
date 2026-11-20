//! Tiles that cycle through frames: water, torches, conveyor belts.

/// A sequence a tile shows instead of its one frame.
///
/// Keyed in [`Tilemap::animations`](super::Tilemap::animations) by the frame
/// it replaces — Tiled's and Godot's model: the level's cells keep naming one
/// frame, and every cell naming it animates, so a level already painted
/// animates without touching a cell.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TileAnimation {
    /// The frames shown in turn, from the layer's atlas.
    pub frames: Vec<u32>,
    /// Seconds each one is shown for.
    pub frame_time: f32,
}

impl TileAnimation {
    /// The frame showing `time` seconds in, or `None` for an animation with
    /// nothing to show — no frames, or a frame time that is not a positive
    /// number — which draws the tile's own frame instead.
    pub fn frame_at(&self, time: f32) -> Option<u32> {
        if self.frames.is_empty()
            || !(self.frame_time.is_finite() && self.frame_time > 0.0)
            || !time.is_finite()
        {
            return None;
        }
        let step = (time.max(0.0) / self.frame_time) as u64;
        Some(self.frames[(step % self.frames.len() as u64) as usize])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn water() -> TileAnimation {
        TileAnimation {
            frames: vec![4, 5, 6],
            frame_time: 0.25,
        }
    }

    #[test]
    fn frames_follow_one_another_and_loop() {
        let water = water();
        assert_eq!(water.frame_at(0.0), Some(4));
        assert_eq!(water.frame_at(0.3), Some(5));
        assert_eq!(water.frame_at(0.6), Some(6));
        assert_eq!(water.frame_at(0.8), Some(4), "and round again");
    }

    #[test]
    fn nothing_to_show_draws_the_tile_s_own_frame() {
        let empty = TileAnimation {
            frames: Vec::new(),
            frame_time: 0.25,
        };
        assert_eq!(empty.frame_at(1.0), None);
        let frozen = TileAnimation {
            frame_time: 0.0,
            ..water()
        };
        assert_eq!(frozen.frame_at(1.0), None);
        assert_eq!(water().frame_at(f32::NAN), None);
    }

    #[test]
    fn a_huge_time_does_not_overflow() {
        assert!(water().frame_at(1.0e30).is_some());
    }
}
