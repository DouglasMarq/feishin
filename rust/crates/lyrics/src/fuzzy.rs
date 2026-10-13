//! Port of the Fuse.js 7 Bitap scoring used by `orderSearchResults` in `shared.ts`.

use std::collections::HashMap;

const MAX_BITS: usize = 32;
const THRESHOLD: f64 = 0.6;
const DISTANCE: f64 = 100.0;

pub struct SearchItem<'a> {
    pub name: &'a str,
    pub artist: &'a str,
    pub is_sync: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ranked {
    pub index: usize,
    /// 0 is a perfect match, 1 the worst.
    pub score: f64,
}

/// Ranks items like `orderSearchResults`: synced first, then by ascending fuzzy score.
pub fn order_search_results(name: &str, artist: &str, items: &[SearchItem<'_>]) -> Vec<Ranked> {
    let mut results = if !artist.is_empty() && !name.is_empty() {
        let artist_results = search_key(artist, items, |item| item.artist, 1.0);
        let name_scores: HashMap<usize, f64> = search_key(name, items, |item| item.name, 1.0)
            .into_iter()
            .map(|r| (r.index, r.score))
            .collect();
        artist_results
            .into_iter()
            .filter_map(|r| {
                name_scores.get(&r.index).map(|&name_score| Ranked {
                    index: r.index,
                    score: r.score.max(name_score),
                })
            })
            .collect()
    } else if !name.is_empty() {
        search_key(name, items, |item| item.name, 0.5)
    } else if !artist.is_empty() {
        search_key(artist, items, |item| item.artist, 0.5)
    } else {
        Vec::new()
    };

    results.sort_by(|a, b| {
        let a_sync = items[a.index].is_sync == Some(true);
        let b_sync = items[b.index].is_sync == Some(true);
        b_sync.cmp(&a_sync).then(a.score.total_cmp(&b.score))
    });
    results
}

fn search_key<'a>(
    pattern: &str,
    items: &[SearchItem<'a>],
    get: impl Fn(&SearchItem<'a>) -> &'a str,
    weight: f64,
) -> Vec<Ranked> {
    let searcher = BitapSearch::new(pattern);
    let mut results: Vec<Ranked> = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            let text = get(item);
            if text.trim().is_empty() {
                return None;
            }
            let score = searcher.search_in(text)?;
            let base = if score == 0.0 { f64::EPSILON } else { score };
            Some(Ranked {
                index,
                score: base.powf(weight * field_norm(text)),
            })
        })
        .collect();
    results.sort_by(|a, b| a.score.total_cmp(&b.score).then(a.index.cmp(&b.index)));
    results
}

fn field_norm(text: &str) -> f64 {
    let mut tokens = 1u32;
    let mut in_space = false;
    for unit in text.encode_utf16() {
        if unit == 32 {
            if !in_space {
                tokens += 1;
                in_space = true;
            }
        } else {
            in_space = false;
        }
    }
    (1000.0 / f64::from(tokens).powf(0.5)).round() / 1000.0
}

struct Chunk {
    pattern: Vec<u16>,
    alphabet: HashMap<u16, i32>,
    start: usize,
}

struct BitapSearch {
    pattern: Vec<u16>,
    chunks: Vec<Chunk>,
}

impl BitapSearch {
    fn new(pattern: &str) -> Self {
        let pattern: Vec<u16> = pattern.to_lowercase().encode_utf16().collect();
        let len = pattern.len();
        let mut chunks = Vec::new();
        let mut add = |start: usize, end: usize| {
            let chunk = pattern[start..end].to_vec();
            chunks.push(Chunk {
                alphabet: alphabet(&chunk),
                pattern: chunk,
                start,
            });
        };
        if len > MAX_BITS {
            let remainder = len % MAX_BITS;
            let end = len - remainder;
            let mut i = 0;
            while i < end {
                add(i, i + MAX_BITS);
                i += MAX_BITS;
            }
            if remainder > 0 {
                add(len - MAX_BITS, len);
            }
        } else if len > 0 {
            add(0, len);
        }
        Self { pattern, chunks }
    }

    /// Returns the score when the text matches.
    fn search_in(&self, text: &str) -> Option<f64> {
        let text: Vec<u16> = text.to_lowercase().encode_utf16().collect();
        if self.pattern == text {
            return Some(0.0);
        }
        let mut total = 0.0;
        let mut matched = false;
        for chunk in &self.chunks {
            let (is_match, score) = bitap(&text, &chunk.pattern, &chunk.alphabet, chunk.start);
            matched |= is_match;
            total += score;
        }
        matched.then(|| total / self.chunks.len() as f64)
    }
}

fn alphabet(pattern: &[u16]) -> HashMap<u16, i32> {
    let len = pattern.len();
    let mut mask = HashMap::new();
    for (i, &unit) in pattern.iter().enumerate() {
        *mask.entry(unit).or_insert(0) |= 1i32.wrapping_shl((len - i - 1) as u32);
    }
    mask
}

fn index_of(text: &[u16], pattern: &[u16], from: usize) -> Option<usize> {
    if from > text.len() {
        return None;
    }
    text[from..]
        .windows(pattern.len())
        .position(|w| w == pattern)
        .map(|p| p + from)
}

fn bitap(
    text: &[u16],
    pattern: &[u16],
    alphabet: &HashMap<u16, i32>,
    location: usize,
) -> (bool, f64) {
    let pattern_len = pattern.len() as i64;
    let text_len = text.len() as i64;
    let expected = (location as i64).min(text_len).max(0);
    let calc = |errors: i64, current: i64| {
        errors as f64 / pattern_len as f64 + (expected - current).abs() as f64 / DISTANCE
    };

    let mut threshold = THRESHOLD;
    let mut from = expected as usize;
    while let Some(index) = index_of(text, pattern, from) {
        threshold = threshold.min(calc(0, index as i64));
        from = index + pattern.len();
    }

    let mut best_location: i64 = -1;
    let mut final_score = 1.0;
    let mut last_bits: Vec<i32> = Vec::new();
    let mut bin_max = pattern_len + text_len;
    let mask = 1i32.wrapping_shl((pattern_len - 1) as u32);
    let at = |v: &[i32], j: i64| -> i32 {
        usize::try_from(j)
            .ok()
            .and_then(|j| v.get(j))
            .copied()
            .unwrap_or(0)
    };

    for i in 0..pattern_len {
        let mut bin_min = 0;
        let mut bin_mid = bin_max;
        while bin_min < bin_mid {
            if calc(i, expected + bin_mid) <= threshold {
                bin_min = bin_mid;
            } else {
                bin_max = bin_mid;
            }
            bin_mid = (bin_max - bin_min) / 2 + bin_min;
        }
        bin_max = bin_mid;

        let mut start = (expected - bin_mid + 1).max(1);
        let finish = (expected + bin_mid).min(text_len) + pattern_len;
        let mut bits = vec![0i32; (finish + 2) as usize];
        bits[(finish + 1) as usize] = 1i32.wrapping_shl(i as u32).wrapping_sub(1);

        let mut j = finish;
        while j >= start {
            let current = j - 1;
            let char_match = usize::try_from(current)
                .ok()
                .and_then(|c| text.get(c))
                .and_then(|unit| alphabet.get(unit))
                .copied()
                .unwrap_or(0);
            let ju = j as usize;
            let mut value = (bits[ju + 1].wrapping_shl(1) | 1) & char_match;
            if i > 0 {
                value |= (at(&last_bits, j + 1) | at(&last_bits, j)).wrapping_shl(1)
                    | 1
                    | at(&last_bits, j + 1);
            }
            bits[ju] = value;
            if value & mask != 0 {
                final_score = calc(i, current);
                if final_score <= threshold {
                    threshold = final_score;
                    best_location = current;
                    if best_location <= expected {
                        break;
                    }
                    start = (2 * expected - best_location).max(1);
                }
            }
            j -= 1;
        }

        if calc(i + 1, expected) > threshold {
            break;
        }
        last_bits = bits;
    }

    (best_location >= 0, final_score.max(0.001))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item<'a>(name: &'a str, artist: &'a str, is_sync: Option<bool>) -> SearchItem<'a> {
        SearchItem {
            name,
            artist,
            is_sync,
        }
    }

    #[test]
    fn exact_match_scores_near_zero() {
        let items = [item("Bohemian Rhapsody", "Queen", None)];
        let ranked = order_search_results("Bohemian Rhapsody", "Queen", &items);
        assert_eq!(ranked.len(), 1);
        assert!(ranked[0].score < 1e-6);
    }

    #[test]
    fn unrelated_items_are_dropped() {
        let items = [item("Completely Different", "Nobody Known", None)];
        assert!(order_search_results("Bohemian Rhapsody", "Queen", &items).is_empty());
    }

    #[test]
    fn empty_query_matches_nothing() {
        let items = [item("Song", "Artist", None)];
        assert!(order_search_results("", "", &items).is_empty());
    }

    type Case<'a> = (
        &'a str,
        &'a str,
        Vec<(&'a str, &'a str, Option<bool>)>,
        Vec<(usize, f64)>,
    );

    /// Expected values produced by running `orderSearchResults` with Fuse.js 7.4.2.
    #[test]
    fn matches_fuse_js_reference_scores() {
        let cases: Vec<Case<'_>> = vec![
            (
                "Bohemian Rhapsody",
                "Queen",
                vec![
                    ("Bohemian Rhapsody", "Queen", None),
                    ("Bohemian Rhapsody (Remastered 2011)", "Queen", Some(true)),
                    ("Bohemain Rapsody", "Queen", Some(false)),
                    ("Bohemian Rhapsody", "Panic! At The Disco", Some(true)),
                    ("Killer Queen", "Queen", None),
                    ("Bohemian Rhapsody by Queen", "Queen", None),
                ],
                vec![
                    (1, 0.03162277660168379),
                    (0, 8.569061098350962e-12),
                    (5, 0.03162277660168379),
                    (2, 0.29335759711558734),
                ],
            ),
            (
                "Lemon",
                "Kenshi Yonezu",
                vec![
                    ("Lemon", "Kenshi Yonezu", None),
                    ("Lemon (Live)", "Kenshi Yonezu, Guest", None),
                    ("Lemon Tree", "Fools Garden", None),
                    ("Lemonade", "Kenshi", None),
                ],
                vec![
                    (0, 8.569061098350962e-12),
                    (1, 0.01857804455091699),
                    (3, 0.5384615384615384),
                ],
            ),
            (
                "A Very Long Song Title That Exceeds Thirty Two Characters",
                "Some Long Artist Name With Many Words In It Too",
                vec![
                    (
                        "A Very Long Song Title That Exceeds Thirty Two Characters",
                        "Some Long Artist Name With Many Words In It Too",
                        Some(true),
                    ),
                    (
                        "A Very Long Song Title That Exceeds 32 Characters (Live)",
                        "Some Long Artist Name With Many Words",
                        None,
                    ),
                    ("Very Long Song", "Some Long Artist", Some(false)),
                ],
                vec![
                    (0, 0.00001131032569734829),
                    (1, 0.556782511481585),
                    (2, 0.8672410917975312),
                ],
            ),
            (
                "Café del Mar",
                "Energy 52",
                vec![
                    ("Café Del Mar", "Energy 52", None),
                    ("Cafe del Mar", "Energy52", None),
                    ("夢ならば", "Energy 52", None),
                ],
                vec![(0, 9.287439764962262e-10), (1, 0.2384033869402615)],
            ),
            (
                "Yesterday",
                "",
                vec![
                    ("Yesterday", "The Beatles", None),
                    ("Yesterday (Remastered)", "The Beatles", Some(true)),
                    ("Yesterdays", "Guns N Roses", None),
                    ("Today", "X", None),
                ],
                vec![
                    (1, 0.08699614330652679),
                    (0, 1.4901161193847656e-8),
                    (2, 0.03162277660168379),
                    (3, 0.7453559924999299),
                ],
            ),
            (
                "",
                "Radiohead",
                vec![
                    ("Creep", "Radiohead", None),
                    ("Karma Police", "Radiohead, Thom", None),
                    ("Creep", "Radio Head", None),
                    ("Creep", "TLC", None),
                ],
                vec![
                    (0, 1.4901161193847656e-8),
                    (1, 0.08699614330652679),
                    (2, 0.459912562959988),
                ],
            ),
            (
                "夢ならばどれほどよかったでしょう",
                "米津玄師",
                vec![
                    ("夢ならばどれほどよかったでしょう", "米津玄師", None),
                    ("夢ならば", "米津 玄師", None),
                ],
                vec![(0, 2.220446049250313e-16)],
            ),
            (
                "Under Pressure",
                "Queen",
                vec![
                    ("Bohemian Rhapsody by Queen", "Queen", None),
                    (
                        "Under Pressure by Queen & David Bowie",
                        "Queen & David Bowie",
                        None,
                    ),
                    ("  ", "Queen", None),
                ],
                vec![(1, 0.07345138681571149)],
            ),
        ];

        for (name, artist, raw_items, expected) in cases {
            let items: Vec<SearchItem<'_>> =
                raw_items.iter().map(|&(n, a, s)| item(n, a, s)).collect();
            let ranked = order_search_results(name, artist, &items);
            let got: Vec<usize> = ranked.iter().map(|r| r.index).collect();
            let want: Vec<usize> = expected.iter().map(|e| e.0).collect();
            assert_eq!(got, want, "order for {name:?} / {artist:?}");
            for (r, (_, score)) in ranked.iter().zip(&expected) {
                let tolerance = score.abs() * 1e-9 + 1e-18;
                assert!(
                    (r.score - score).abs() <= tolerance,
                    "{name:?}/{artist:?} item {}: got {}, want {score}",
                    r.index,
                    r.score
                );
            }
        }
    }
}
