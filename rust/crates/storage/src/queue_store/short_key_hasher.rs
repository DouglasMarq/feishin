use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

const GOLDEN_RATIO_MULTIPLIER: u64 = 0x9E37_79B9_7F4A_7C15;
const MIX_ROTATION: u32 = 5;
const FINISH_ROTATION: u32 = 26;
const WORD_BYTES: usize = 8;

pub(super) type ShortKeyMap<Key, Value> = HashMap<Key, Value, BuildHasherDefault<ShortKeyHasher>>;

#[derive(Debug, Default)]
pub(super) struct ShortKeyHasher {
    state: u64,
}

impl Hasher for ShortKeyHasher {
    fn finish(&self) -> u64 {
        self.state.rotate_left(FINISH_ROTATION)
    }

    fn write(&mut self, bytes: &[u8]) {
        let (words, remainder) = bytes.as_chunks::<WORD_BYTES>();
        for word in words {
            self.mix(u64::from_le_bytes(*word));
        }
        if !remainder.is_empty() {
            self.mix_remainder(remainder);
        }
    }

    fn write_u8(&mut self, value: u8) {
        self.mix(u64::from(value));
    }

    fn write_usize(&mut self, value: usize) {
        self.mix(value as u64);
    }
}

impl ShortKeyHasher {
    fn mix_remainder(&mut self, bytes: &[u8]) {
        let mut word = [0_u8; WORD_BYTES];
        word.iter_mut()
            .zip(bytes)
            .for_each(|(slot, byte)| *slot = *byte);
        self.mix(u64::from_le_bytes(word));
    }

    fn mix(&mut self, value: u64) {
        self.state =
            (self.state.rotate_left(MIX_ROTATION) ^ value).wrapping_mul(GOLDEN_RATIO_MULTIPLIER);
    }
}

#[cfg(test)]
mod tests {
    use std::hash::{BuildHasher, Hash};

    use super::*;

    fn hash_of(value: impl Hash) -> u64 {
        BuildHasherDefault::<ShortKeyHasher>::default().hash_one(value)
    }

    #[test]
    fn different_short_keys_hash_differently() {
        assert_ne!(hash_of("album-1"), hash_of("album-2"));
        assert_ne!(hash_of(("ab", "c")), hash_of(("a", "bc")));
        assert_ne!(hash_of("a long album title"), hash_of("a long album titlf"));
        assert_eq!(hash_of("same"), hash_of("same"));
    }
}
