use std::hash::{BuildHasher, RandomState};

const GOLDEN_RATIO_INCREMENT: u64 = 0x9E37_79B9_7F4A_7C15;
const FIRST_MIX_MULTIPLIER: u64 = 0xBF58_476D_1CE4_E5B9;
const SECOND_MIX_MULTIPLIER: u64 = 0x94D0_49BB_1331_11EB;
const SEED_HASH_INPUT: u64 = 0x5EED;

#[derive(Debug, Clone)]
pub(super) struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub(super) fn from_seed(seed: u64) -> Self {
        Self { state: seed }
    }

    pub(super) fn from_random_seed() -> Self {
        Self::from_seed(RandomState::new().hash_one(SEED_HASH_INPUT))
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GOLDEN_RATIO_INCREMENT);
        let mut mixed = self.state;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(FIRST_MIX_MULTIPLIER);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(SECOND_MIX_MULTIPLIER);
        mixed ^ (mixed >> 31)
    }

    fn index_below(&mut self, exclusive_bound: usize) -> usize {
        ((u128::from(self.next_u64()) * exclusive_bound as u128) >> 64) as usize
    }

    pub(super) fn shuffle<Element>(&mut self, elements: &mut [Element]) {
        for last_unshuffled in (1..elements.len()).rev() {
            let swap_with = self.index_below(last_unshuffled + 1);
            elements.swap(last_unshuffled, swap_with);
        }
    }
}
