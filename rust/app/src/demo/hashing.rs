pub(super) fn scramble(value: u64) -> u64 {
    let mut scrambled = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    scrambled = (scrambled ^ (scrambled >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    scrambled = (scrambled ^ (scrambled >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    scrambled ^ (scrambled >> 31)
}

pub(super) fn pick_word<'word>(words: &[&'word str], hash: u64) -> &'word str {
    let position = (hash % words.len() as u64) as usize;
    words.get(position).copied().unwrap_or_default()
}
