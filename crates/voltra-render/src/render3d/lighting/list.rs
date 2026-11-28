//! Which of a frame's lights reach one instance: what its instance
//! attribute carries.

use super::{LIGHTS_PER_INSTANCE, MAX_DIRECTIONAL, MAX_PUNCTUAL};

/// The lights one instance is lit by: a bit per directional light of the
/// frame's [`Lighting3D::directional`](super::Lighting3D::directional),
/// and up to [`LIGHTS_PER_INSTANCE`] indices into its
/// [`Lighting3D::punctual`](super::Lighting3D::punctual) — each a `u8`,
/// four to a `u32`, [`NONE`](Self::NONE) for an empty slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LightList3D {
    directional: u8,
    punctual: [u8; LIGHTS_PER_INSTANCE],
}

impl Default for LightList3D {
    /// Every directional light, no point or spot light: what an instance
    /// nobody chose lights for is lit by.
    fn default() -> Self {
        Self::ALL_DIRECTIONAL
    }
}

impl LightList3D {
    /// An empty slot.
    pub const NONE: u8 = u8::MAX;

    /// No light at all.
    pub const EMPTY: Self = Self {
        directional: 0,
        punctual: [Self::NONE; LIGHTS_PER_INSTANCE],
    };

    /// Every directional light, no point or spot light.
    pub const ALL_DIRECTIONAL: Self = Self {
        directional: (1 << MAX_DIRECTIONAL) - 1,
        punctual: [Self::NONE; LIGHTS_PER_INSTANCE],
    };

    /// The directional lights whose bit is set in `directional` (bit `i`
    /// the frame's `i`th; past [`MAX_DIRECTIONAL`] ignored) and the point
    /// and spot lights at `punctual` — the first [`LIGHTS_PER_INSTANCE`],
    /// an index past [`MAX_PUNCTUAL`] left out.
    pub fn new(directional: u8, punctual: &[u8]) -> Self {
        let mut list = Self {
            directional: directional & Self::ALL_DIRECTIONAL.directional,
            ..Self::EMPTY
        };
        let valid = punctual.iter().filter(|&&i| usize::from(i) < MAX_PUNCTUAL);
        for (slot, &index) in list.punctual.iter_mut().zip(valid) {
            *slot = index;
        }
        list
    }

    /// The directional lights' bits.
    pub fn directional(&self) -> u8 {
        self.directional
    }

    /// The point and spot lights' indices, empty slots left out.
    pub fn punctual(&self) -> impl Iterator<Item = u8> + '_ {
        self.punctual.iter().copied().filter(|&i| i != Self::NONE)
    }

    /// The attribute the shader reads: the eight indices four to a word,
    /// the first in each word's low byte, then the directional bits.
    pub fn packed(&self) -> [u32; 3] {
        let word = |at: usize| u32::from_le_bytes(std::array::from_fn(|i| self.punctual[at + i]));
        [word(0), word(4), u32::from(self.directional)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indices_pack_four_to_a_word_and_an_empty_slot_reads_none() {
        let list = LightList3D::new(0b101, &[3, 1, 200, 127, 0, 9]);
        assert_eq!(list.punctual().collect::<Vec<_>>(), [3, 1, 127, 0, 9]);
        let [low, high, directional] = list.packed();
        assert_eq!(low, 3 | 1 << 8 | 127 << 16);
        assert_eq!(
            low >> 24,
            0,
            "the fourth index in the first word's top byte"
        );
        assert_eq!(high & 0xFF, 9, "the fifth in the second word's low byte");
        assert_eq!(high >> 8, 0x00FF_FFFF, "the rest are empty");
        assert_eq!(directional, 0b101);
        let many: Vec<u8> = (0..12).collect();
        assert_eq!(LightList3D::new(0, &many).punctual().count(), 8);
        assert_eq!(LightList3D::new(0xFF, &[]).directional(), 0b1111);
        assert_eq!(
            LightList3D::default().packed(),
            [u32::MAX, u32::MAX, 0b1111]
        );
        assert_eq!(LightList3D::EMPTY.packed(), [u32::MAX, u32::MAX, 0]);
    }
}
