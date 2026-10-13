use std::collections::HashMap;
use std::ops::Range;

const MAX_BITS: usize = 32;
const THRESHOLD: f64 = 0.6;
const DISTANCE: f64 = 100.0;
const MINIMUM_SCORE: f64 = 0.001;

type Alphabet = HashMap<u16, i32>;

struct Chunk {
    pattern: Vec<u16>,
    alphabet: Alphabet,
    start: usize,
}

pub(super) struct BitapSearch {
    pattern: Vec<u16>,
    chunks: Vec<Chunk>,
}

impl BitapSearch {
    pub(super) fn new(pattern: &str) -> Self {
        let pattern: Vec<u16> = pattern.to_lowercase().encode_utf16().collect();
        let chunks = chunk_ranges(pattern.len())
            .into_iter()
            .filter_map(|range| {
                let start = range.start;
                let chunk_pattern = pattern.get(range)?.to_vec();
                Some(Chunk {
                    alphabet: alphabet(&chunk_pattern),
                    pattern: chunk_pattern,
                    start,
                })
            })
            .collect();
        Self { pattern, chunks }
    }

    pub(super) fn search_in(&self, text: &str) -> Option<f64> {
        let text: Vec<u16> = text.to_lowercase().encode_utf16().collect();
        if self.pattern == text {
            return Some(0.0);
        }
        let mut total = 0.0;
        let mut matched = false;
        for chunk in &self.chunks {
            let chunk_match = bitap(&text, &chunk.pattern, &chunk.alphabet, chunk.start);
            matched |= chunk_match.is_match;
            total += chunk_match.score;
        }
        matched.then(|| total / self.chunks.len() as f64)
    }
}

fn chunk_ranges(length: usize) -> Vec<Range<usize>> {
    match length {
        0 => Vec::new(),
        1..=MAX_BITS => std::iter::once(0..length).collect(),
        _ => {
            let remainder = length % MAX_BITS;
            let full_chunks_end = length - remainder;
            let mut ranges: Vec<Range<usize>> = (0..full_chunks_end)
                .step_by(MAX_BITS)
                .map(|start| start..start + MAX_BITS)
                .collect();
            if remainder > 0 {
                ranges.push(length - MAX_BITS..length);
            }
            ranges
        }
    }
}

fn alphabet(pattern: &[u16]) -> Alphabet {
    let length = pattern.len();
    let mut mask = HashMap::new();
    for (index, &unit) in pattern.iter().enumerate() {
        *mask.entry(unit).or_insert(0) |= 1i32.wrapping_shl((length - index - 1) as u32);
    }
    mask
}

fn index_of(text: &[u16], pattern: &[u16], from: usize) -> Option<usize> {
    if pattern.is_empty() {
        return None;
    }
    text.get(from..)?
        .windows(pattern.len())
        .position(|window| window == pattern)
        .map(|position| position + from)
}

struct Scorer {
    pattern_length: i64,
    text_length: i64,
    expected_location: i64,
}

impl Scorer {
    fn new(pattern: &[u16], text: &[u16], location: usize) -> Self {
        let text_length = text.len() as i64;
        Self {
            pattern_length: pattern.len() as i64,
            text_length,
            expected_location: (location as i64).min(text_length).max(0),
        }
    }

    fn score(&self, errors: i64, current_location: i64) -> f64 {
        errors as f64 / self.pattern_length as f64
            + (self.expected_location - current_location).abs() as f64 / DISTANCE
    }
}

struct ChunkMatch {
    is_match: bool,
    score: f64,
}

struct MatchState {
    threshold: f64,
    best_location: i64,
    final_score: f64,
    bin_max: i64,
}

enum ScanStep {
    Continue,
    Stop,
    NarrowStart(i64),
}

impl MatchState {
    fn narrow_search_radius(&mut self, scorer: &Scorer, errors: i64) -> i64 {
        let mut bin_min = 0;
        let mut bin_mid = self.bin_max;
        while bin_min < bin_mid {
            match scorer.score(errors, scorer.expected_location + bin_mid) <= self.threshold {
                true => bin_min = bin_mid,
                false => self.bin_max = bin_mid,
            }
            bin_mid = (self.bin_max - bin_min) / 2 + bin_min;
        }
        self.bin_max = bin_mid;
        bin_mid
    }

    fn record_match(&mut self, scorer: &Scorer, errors: i64, current_location: i64) -> ScanStep {
        self.final_score = scorer.score(errors, current_location);
        if self.final_score > self.threshold {
            return ScanStep::Continue;
        }
        self.threshold = self.final_score;
        self.best_location = current_location;
        if current_location <= scorer.expected_location {
            return ScanStep::Stop;
        }
        ScanStep::NarrowStart((2 * scorer.expected_location - current_location).max(1))
    }
}

struct RowScan<'search> {
    text: &'search [u16],
    alphabet: &'search Alphabet,
    scorer: &'search Scorer,
    last_bits: &'search [i32],
    match_mask: i32,
    errors: i64,
}

impl RowScan<'_> {
    fn scan(&self, state: &mut MatchState, radius: i64) -> Vec<i32> {
        let expected_location = self.scorer.expected_location;
        let mut start = (expected_location - radius + 1).max(1);
        let finish =
            (expected_location + radius).min(self.scorer.text_length) + self.scorer.pattern_length;
        let mut bits = vec![0i32; usize::try_from(finish + 2).unwrap_or_default()];
        let initial_bits = 1i32.wrapping_shl(self.errors as u32).wrapping_sub(1);
        set_bit(&mut bits, finish + 1, initial_bits);
        let mut position = finish;
        while position >= start {
            let value = self.row_value(&bits, position);
            set_bit(&mut bits, position, value);
            if value & self.match_mask != 0 {
                match state.record_match(self.scorer, self.errors, position - 1) {
                    ScanStep::Continue => {}
                    ScanStep::Stop => break,
                    ScanStep::NarrowStart(narrowed_start) => start = narrowed_start,
                }
            }
            position -= 1;
        }
        bits
    }

    fn row_value(&self, bits: &[i32], position: i64) -> i32 {
        let character_match = usize::try_from(position - 1)
            .ok()
            .and_then(|text_index| self.text.get(text_index))
            .and_then(|unit| self.alphabet.get(unit))
            .copied()
            .unwrap_or(0);
        let value = (bit_at(bits, position + 1).wrapping_shl(1) | 1) & character_match;
        match self.errors {
            0 => value,
            _ => {
                let next = bit_at(self.last_bits, position + 1);
                let current = bit_at(self.last_bits, position);
                value | (next | current).wrapping_shl(1) | 1 | next
            }
        }
    }
}

fn bit_at(bits: &[i32], position: i64) -> i32 {
    usize::try_from(position)
        .ok()
        .and_then(|index| bits.get(index))
        .copied()
        .unwrap_or(0)
}

fn set_bit(bits: &mut [i32], position: i64, value: i32) {
    if let Some(slot) = usize::try_from(position)
        .ok()
        .and_then(|index| bits.get_mut(index))
    {
        *slot = value;
    }
}

fn exact_match_threshold(text: &[u16], pattern: &[u16], scorer: &Scorer) -> f64 {
    let mut threshold = THRESHOLD;
    let mut from = scorer.expected_location as usize;
    while let Some(index) = index_of(text, pattern, from) {
        threshold = threshold.min(scorer.score(0, index as i64));
        from = index + pattern.len();
    }
    threshold
}

fn bitap(text: &[u16], pattern: &[u16], alphabet: &Alphabet, location: usize) -> ChunkMatch {
    let scorer = Scorer::new(pattern, text, location);
    let mut state = MatchState {
        threshold: exact_match_threshold(text, pattern, &scorer),
        best_location: -1,
        final_score: 1.0,
        bin_max: scorer.pattern_length + scorer.text_length,
    };
    let match_mask = 1i32.wrapping_shl((scorer.pattern_length - 1) as u32);
    let mut last_bits: Vec<i32> = Vec::new();
    for errors in 0..scorer.pattern_length {
        let radius = state.narrow_search_radius(&scorer, errors);
        let row_scan = RowScan {
            text,
            alphabet,
            scorer: &scorer,
            last_bits: &last_bits,
            match_mask,
            errors,
        };
        let bits = row_scan.scan(&mut state, radius);
        if scorer.score(errors + 1, scorer.expected_location) > state.threshold {
            break;
        }
        last_bits = bits;
    }
    ChunkMatch {
        is_match: state.best_location >= 0,
        score: state.final_score.max(MINIMUM_SCORE),
    }
}
