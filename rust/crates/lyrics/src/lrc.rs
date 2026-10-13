use crate::LyricLine;

const METADATA_TAGS: &[&str] = &[
    "#", "al", "ar", "au", "by", "id", "la", "lang", "length", "lr", "offset", "re", "tool", "ti",
    "ve",
];

enum Tag<'a> {
    Time(u64),
    Karaoke(u64),
    Metadata { key: &'a str, value: &'a str },
}

pub fn parse_lrc(text: &str) -> Vec<LyricLine> {
    let raw_lines: Vec<&str> = text.lines().collect();
    let mut offset_ms: i64 = 0;
    let mut timed = Vec::new();
    let mut karaoke = Vec::new();

    for line in &raw_lines {
        let (tags, rest) = leading_tags(line.trim_start());
        let mut times = Vec::new();
        let mut karaoke_time = None;
        for tag in tags {
            match tag {
                Tag::Time(ms) => times.push(ms),
                Tag::Karaoke(ms) => karaoke_time = Some(ms),
                Tag::Metadata { key, value } if key.eq_ignore_ascii_case("offset") => {
                    if let Ok(value) = value.trim().trim_start_matches('+').parse::<i64>() {
                        offset_ms = value;
                    }
                }
                Tag::Metadata { .. } => {}
            }
        }
        if !times.is_empty() {
            let text = strip_word_times(rest).trim().to_owned();
            timed.extend(times.into_iter().map(|ms| (ms, text.clone())));
        } else if let Some(ms) = karaoke_time {
            karaoke.push((ms, clean_karaoke_text(rest)));
        }
    }

    let mut synced = if timed.is_empty() { karaoke } else { timed };
    if synced.is_empty() {
        return plain_lines(&raw_lines);
    }

    synced.sort_by_key(|(ms, _)| *ms);
    synced
        .into_iter()
        .map(|(ms, text)| LyricLine {
            time_ms: Some(apply_offset(ms, offset_ms)),
            text,
        })
        .collect()
}

fn apply_offset(ms: u64, offset_ms: i64) -> u64 {
    let shifted = i128::from(ms) - i128::from(offset_ms);
    u64::try_from(shifted.max(0)).unwrap_or(u64::MAX)
}

fn plain_lines(raw_lines: &[&str]) -> Vec<LyricLine> {
    let kept: Vec<&str> = raw_lines
        .iter()
        .filter(|line| !is_metadata_line(line))
        .map(|line| line.trim_end())
        .collect();
    let start = kept.iter().position(|l| !l.trim().is_empty());
    let end = kept.iter().rposition(|l| !l.trim().is_empty());
    match (start, end) {
        (Some(start), Some(end)) => kept[start..=end]
            .iter()
            .map(|text| LyricLine {
                time_ms: None,
                text: (*text).to_owned(),
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn is_metadata_line(line: &str) -> bool {
    let trimmed = line.trim();
    let (tags, rest) = leading_tags(trimmed);
    rest.trim().is_empty()
        && !tags.is_empty()
        && tags.iter().all(|tag| matches!(tag, Tag::Metadata { .. }))
}

fn leading_tags(mut line: &str) -> (Vec<Tag<'_>>, &str) {
    let mut tags = Vec::new();
    while let Some(inner) = line.strip_prefix('[') {
        let Some(close) = inner.find(']') else {
            break;
        };
        let Some(tag) = parse_tag(&inner[..close]) else {
            break;
        };
        tags.push(tag);
        line = &inner[close + 1..];
    }
    (tags, line)
}

fn parse_tag(content: &str) -> Option<Tag<'_>> {
    if let Some(ms) = parse_timestamp(content) {
        return Some(Tag::Time(ms));
    }
    if let Some((start, duration)) = content.split_once(',')
        && is_digits(start)
        && (duration.is_empty() || is_digits(duration))
    {
        return start.parse().ok().map(Tag::Karaoke);
    }
    let (key, value) = content.split_once(':')?;
    let key = key.trim();
    METADATA_TAGS
        .iter()
        .any(|known| known.eq_ignore_ascii_case(key))
        .then_some(Tag::Metadata { key, value })
}

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// Parses `mm:ss`, `mm:ss.f`, `mm:ss.ff`, `mm:ss.fff` and `mm:ss:ff`.
fn parse_timestamp(content: &str) -> Option<u64> {
    let (minutes, rest) = content.trim().split_once(':')?;
    let (seconds, fraction) = match rest.split_once(['.', ':']) {
        Some((seconds, fraction)) => (seconds, Some(fraction)),
        None => (rest, None),
    };
    if !is_digits(minutes) || !is_digits(seconds) || seconds.len() > 2 {
        return None;
    }
    let minutes: u64 = minutes.parse().ok()?;
    let seconds: u64 = seconds.parse().ok()?;
    let millis = match fraction {
        None => 0,
        Some(f) if is_digits(f) && f.len() <= 3 => {
            let value: u64 = f.parse().ok()?;
            value * 10u64.pow(3 - u32::try_from(f.len()).ok()?)
        }
        Some(_) => return None,
    };
    minutes
        .checked_mul(60)?
        .checked_add(seconds)?
        .checked_mul(1000)?
        .checked_add(millis)
}

fn strip_word_times(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('<') {
        let after = &rest[open + 1..];
        match after.find('>') {
            Some(close) if parse_timestamp(&after[..close]).is_some() => {
                out.push_str(&rest[..open]);
                rest = &after[close + 1..];
            }
            _ => {
                out.push_str(&rest[..=open]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Port of the NetEase karaoke cleanup: drops `(start,duration)` word cues and the space they leave before punctuation.
fn clean_karaoke_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('(') {
        let after = &rest[open + 1..];
        let cue = after.find(')').and_then(|close| {
            let (a, b) = after[..close].split_once(',')?;
            (is_digits(a) && is_digits(b)).then_some(close)
        });
        match cue {
            Some(close) => {
                out.push_str(&rest[..open]);
                rest = &after[close + 1..];
            }
            None => {
                out.push_str(&rest[..=open]);
                rest = after;
            }
        }
    }
    out.push_str(rest);

    let mut cleaned = String::with_capacity(out.len());
    let mut chars = out.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_whitespace() && matches!(chars.peek(), Some(',' | '.')) {
            continue;
        }
        cleaned.push(c);
    }
    cleaned
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synced(lines: &[(u64, &str)]) -> Vec<LyricLine> {
        lines
            .iter()
            .map(|(ms, text)| LyricLine {
                time_ms: Some(*ms),
                text: (*text).to_owned(),
            })
            .collect()
    }

    fn unsynced(lines: &[&str]) -> Vec<LyricLine> {
        lines
            .iter()
            .map(|text| LyricLine {
                time_ms: None,
                text: (*text).to_owned(),
            })
            .collect()
    }

    #[test]
    fn parses_fraction_precisions() {
        let lrc = "[00:01]a\n[00:02.5]b\n[00:03.25]c\n[00:04.125]d\n[01:05:50]e";
        assert_eq!(
            parse_lrc(lrc),
            synced(&[
                (1000, "a"),
                (2500, "b"),
                (3250, "c"),
                (4125, "d"),
                (65500, "e")
            ])
        );
    }

    #[test]
    fn expands_multiple_timestamps_and_sorts() {
        let lrc = "[00:10.00][00:30.00]chorus\n[00:20.00]verse\n[00:05.00]intro";
        assert_eq!(
            parse_lrc(lrc),
            synced(&[
                (5000, "intro"),
                (10000, "chorus"),
                (20000, "verse"),
                (30000, "chorus")
            ])
        );
    }

    #[test]
    fn skips_metadata_and_applies_offset() {
        let lrc = "[ar:Artist]\n[ti:Title]\n[al:Album]\n[by:someone]\n[length: 03:20]\n[offset:+500]\n[00:01.00]one\n[00:00.20]zero";
        assert_eq!(parse_lrc(lrc), synced(&[(0, "zero"), (500, "one")]));
    }

    #[test]
    fn negative_offset_delays_lines() {
        assert_eq!(
            parse_lrc("[offset:-250]\n[00:01.00]x"),
            synced(&[(1250, "x")])
        );
    }

    #[test]
    fn handles_crlf_blank_and_long_minutes() {
        let lrc = "[00:01.00] first \r\n[00:02.00] \r\n\r\n[100:00.00]late\r\n";
        assert_eq!(
            parse_lrc(lrc),
            synced(&[(1000, "first"), (2000, ""), (6_000_000, "late")])
        );
    }

    #[test]
    fn ignores_untimed_lines_in_synced_text() {
        let lrc = "{\"t\":0,\"c\":[{\"tx\":\"credits\"}]}\n[00:01.00]a\nstray line\n[xx:yy]b";
        assert_eq!(parse_lrc(lrc), synced(&[(1000, "a")]));
    }

    #[test]
    fn strips_enhanced_word_timestamps() {
        assert_eq!(
            parse_lrc("[00:01.00]<00:01.00>hello <00:01.50>world <b>"),
            synced(&[(1000, "hello world <b>")])
        );
    }

    #[test]
    fn parses_netease_karaoke_format() {
        let lrc = "[1000,500](1000,200)Hello (1200,300)world ,(1500,100)ok\n[500,400]first";
        assert_eq!(
            parse_lrc(lrc),
            synced(&[(500, "first"), (1000, "Hello world,ok")])
        );
    }

    #[test]
    fn plain_text_becomes_unsynced_lines() {
        let text = "\n[Verse 1]\nLine one\r\n\n[Chorus: Someone]\nLine two\n\n";
        assert_eq!(
            parse_lrc(text),
            unsynced(&["[Verse 1]", "Line one", "", "[Chorus: Someone]", "Line two"])
        );
    }

    #[test]
    fn plain_text_drops_lrc_metadata() {
        assert_eq!(
            parse_lrc("[ar:Artist]\n[ti:Title]\nOnly words"),
            unsynced(&["Only words"])
        );
    }

    #[test]
    fn empty_input_yields_no_lines() {
        assert!(parse_lrc("").is_empty());
        assert!(parse_lrc("  \n\n").is_empty());
    }

    #[test]
    fn rejects_malformed_timestamps() {
        assert_eq!(
            parse_lrc("[00:1234]x\n[00:01.1234]y"),
            unsynced(&["[00:1234]x", "[00:01.1234]y"])
        );
    }
}
