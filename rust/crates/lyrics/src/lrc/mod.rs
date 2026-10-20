mod tags;
mod word_cues;

use crate::{LyricLine, Lyrics, LyricsSource};
use tags::{Tag, leading_tags};
use word_cues::{clean_karaoke_text, strip_word_times};

type TimedText = (u64, String);

#[derive(Default)]
struct CollectedLines {
    offset_ms: i64,
    timed: Vec<TimedText>,
    karaoke: Vec<TimedText>,
}

pub fn parse_lrc(text: &str) -> Vec<LyricLine> {
    let raw_lines: Vec<&str> = text.lines().collect();
    let mut collected = CollectedLines::default();
    for line in &raw_lines {
        collected.add_line(line);
    }
    let offset_ms = collected.offset_ms;
    let mut synced = collected.into_preferred_lines();
    if synced.is_empty() {
        return plain_lines(&raw_lines);
    }
    synced.sort_by_key(|(time_ms, _)| *time_ms);
    synced
        .into_iter()
        .map(|(time_ms, text)| LyricLine {
            time_ms: Some(apply_offset(time_ms, offset_ms)),
            text,
        })
        .collect()
}

pub(crate) fn lyrics_from_text(source: LyricsSource, text: &str) -> Option<Lyrics> {
    let lines = parse_lrc(text);
    if lines.is_empty() {
        return None;
    }
    let is_synced = lines.iter().any(|line| line.time_ms.is_some());
    Some(Lyrics {
        source,
        is_synced,
        lines,
    })
}

impl CollectedLines {
    fn add_line(&mut self, line: &str) {
        let (tags, rest) = leading_tags(line.trim_start());
        let mut times = Vec::new();
        let mut karaoke_time = None;
        for tag in tags {
            match tag {
                Tag::Time(time_ms) => times.push(time_ms),
                Tag::Karaoke(time_ms) => karaoke_time = Some(time_ms),
                Tag::Metadata { key, value } if key.eq_ignore_ascii_case("offset") => {
                    self.apply_offset_tag(value);
                }
                Tag::Metadata { .. } => {}
            }
        }
        match (times.is_empty(), karaoke_time) {
            (false, _) => self.add_timed(&times, rest),
            (true, Some(time_ms)) => self.karaoke.push((time_ms, clean_karaoke_text(rest))),
            (true, None) => {}
        }
    }

    fn apply_offset_tag(&mut self, value: &str) {
        self.offset_ms = value
            .trim()
            .trim_start_matches('+')
            .parse::<i64>()
            .unwrap_or(self.offset_ms);
    }

    fn add_timed(&mut self, times: &[u64], rest: &str) {
        let text = strip_word_times(rest).trim().to_owned();
        self.timed
            .extend(times.iter().map(|&time_ms| (time_ms, text.clone())));
    }

    fn into_preferred_lines(self) -> Vec<TimedText> {
        match self.timed.is_empty() {
            true => self.karaoke,
            false => self.timed,
        }
    }
}

fn apply_offset(time_ms: u64, offset_ms: i64) -> u64 {
    let shifted = i128::from(time_ms) - i128::from(offset_ms);
    u64::try_from(shifted.max(0)).unwrap_or(u64::MAX)
}

fn plain_lines(raw_lines: &[&str]) -> Vec<LyricLine> {
    let kept: Vec<&str> = raw_lines
        .iter()
        .filter(|line| !is_metadata_line(line))
        .map(|line| line.trim_end())
        .collect();
    let is_not_blank = |line: &&str| !line.trim().is_empty();
    let first = kept.iter().position(is_not_blank);
    let last = kept.iter().rposition(is_not_blank);
    let content = match (first, last) {
        (Some(first), Some(last)) => kept.get(first..=last).unwrap_or_default(),
        _ => &[],
    };
    content
        .iter()
        .map(|text| LyricLine {
            time_ms: None,
            text: (*text).to_owned(),
        })
        .collect()
}

fn is_metadata_line(line: &str) -> bool {
    let (tags, rest) = leading_tags(line.trim());
    rest.trim().is_empty()
        && !tags.is_empty()
        && tags.iter().all(|tag| matches!(tag, Tag::Metadata { .. }))
}

#[cfg(test)]
mod tests;
