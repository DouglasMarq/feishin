mod bitap;

use std::collections::HashMap;

use bitap::BitapSearch;

pub(crate) const MATCH_THRESHOLD: f64 = 0.55;
const SINGLE_KEY_WEIGHT: f64 = 0.5;
const COMBINED_KEY_WEIGHT: f64 = 1.0;
const SPACE: u16 = 32;

pub struct SearchItem<'text> {
    pub name: &'text str,
    pub artist: &'text str,
    pub is_sync: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ranked {
    pub index: usize,
    pub score: f64,
}

pub fn order_search_results(name: &str, artist: &str, items: &[SearchItem<'_>]) -> Vec<Ranked> {
    let mut results = match (name.is_empty(), artist.is_empty()) {
        (false, false) => combined_results(name, artist, items),
        (false, true) => search_key(name, items, |item| item.name, SINGLE_KEY_WEIGHT),
        (true, false) => search_key(artist, items, |item| item.artist, SINGLE_KEY_WEIGHT),
        (true, true) => Vec::new(),
    };
    results.sort_by(|left, right| {
        let left_synced = is_synced(items, left.index);
        let right_synced = is_synced(items, right.index);
        right_synced
            .cmp(&left_synced)
            .then(left.score.total_cmp(&right.score))
    });
    results
}

fn combined_results(name: &str, artist: &str, items: &[SearchItem<'_>]) -> Vec<Ranked> {
    let artist_results = search_key(artist, items, |item| item.artist, COMBINED_KEY_WEIGHT);
    let name_scores: HashMap<usize, f64> =
        search_key(name, items, |item| item.name, COMBINED_KEY_WEIGHT)
            .into_iter()
            .map(|ranked| (ranked.index, ranked.score))
            .collect();
    artist_results
        .into_iter()
        .filter_map(|ranked| {
            name_scores.get(&ranked.index).map(|&name_score| Ranked {
                index: ranked.index,
                score: ranked.score.max(name_score),
            })
        })
        .collect()
}

fn is_synced(items: &[SearchItem<'_>], index: usize) -> bool {
    items
        .get(index)
        .is_some_and(|item| item.is_sync == Some(true))
}

fn search_key<'text>(
    pattern: &str,
    items: &[SearchItem<'text>],
    field: impl Fn(&SearchItem<'text>) -> &'text str,
    weight: f64,
) -> Vec<Ranked> {
    let searcher = BitapSearch::new(pattern);
    let mut results: Vec<Ranked> = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            let text = field(item);
            if text.trim().is_empty() {
                return None;
            }
            let score = searcher.search_in(text)?;
            let base = Some(score)
                .filter(|score| *score != 0.0)
                .unwrap_or(f64::EPSILON);
            Some(Ranked {
                index,
                score: base.powf(weight * field_norm(text)),
            })
        })
        .collect();
    results.sort_by(|left, right| {
        left.score
            .total_cmp(&right.score)
            .then(left.index.cmp(&right.index))
    });
    results
}

fn field_norm(text: &str) -> f64 {
    let (tokens, _) =
        text.encode_utf16()
            .fold((1u32, false), |(tokens, previous_was_space), unit| {
                let is_space = unit == SPACE;
                let starts_new_token = is_space && !previous_was_space;
                (tokens + u32::from(starts_new_token), is_space)
            });
    (1000.0 / f64::from(tokens).powf(0.5)).round() / 1000.0
}

#[cfg(test)]
mod tests;
