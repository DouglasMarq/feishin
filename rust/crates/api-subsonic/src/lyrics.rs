use crate::types::StructuredLyric;

fn is_main_lyric(lyric: &StructuredLyric) -> bool {
    lyric.kind.as_deref().is_none_or(|kind| kind == "main")
}

fn lrc_timestamp(milliseconds: u64) -> String {
    let centiseconds = milliseconds / 10;
    format!(
        "[{:02}:{:02}.{:02}]",
        centiseconds / 6000,
        (centiseconds / 100) % 60,
        centiseconds % 100
    )
}

fn preferred_lyric(lyrics: &[StructuredLyric]) -> Option<&StructuredLyric> {
    lyrics
        .iter()
        .find(|lyric| lyric.synced && is_main_lyric(lyric))
        .or_else(|| lyrics.iter().find(|lyric| is_main_lyric(lyric)))
        .or_else(|| lyrics.first())
}

fn has_only_blank_lines(lyric: &StructuredLyric) -> bool {
    lyric.line.iter().all(|line| line.value.trim().is_empty())
}

fn plain_text(lyric: &StructuredLyric) -> String {
    let lines: Vec<&str> = lyric.line.iter().map(|line| line.value.as_str()).collect();
    lines.join("\n")
}

fn lrc_offset_tag(lyric: &StructuredLyric) -> Option<String> {
    lyric
        .offset
        .filter(|offset| offset.is_finite() && *offset != 0.0)
        .map(|offset| format!("[offset:{}]", offset.round() as i64))
}

fn lrc_text(lyric: &StructuredLyric) -> String {
    let timed_lines = lyric.line.iter().map(|line| {
        let start = line
            .start
            .filter(|start| start.is_finite() && *start > 0.0)
            .map_or(0, |start| start.round() as u64);
        format!("{}{}", lrc_timestamp(start), line.value)
    });
    let lines: Vec<String> = lrc_offset_tag(lyric)
        .into_iter()
        .chain(timed_lines)
        .collect();
    lines.join("\n")
}

pub fn lyrics_to_lrc_or_plain_text(lyrics: &[StructuredLyric]) -> Option<String> {
    let lyric = preferred_lyric(lyrics)?;
    match (has_only_blank_lines(lyric), lyric.synced) {
        (true, _) => None,
        (false, true) => Some(lrc_text(lyric)),
        (false, false) => Some(plain_text(lyric)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::LyricLine;

    fn lyric(synced: bool, kind: Option<&str>, lines: &[(u64, &str)]) -> StructuredLyric {
        StructuredLyric {
            kind: kind.map(Into::into),
            lang: Some("eng".into()),
            line: lines
                .iter()
                .map(|(start, value)| LyricLine {
                    start: Some(*start as f64),
                    value: (*value).into(),
                })
                .collect(),
            offset: None,
            synced,
        }
    }

    #[test]
    fn formats_synced_as_lrc() {
        let mut synced = lyric(true, None, &[(0, "a"), (61_234, "b"), (3_600_000, "c")]);
        synced.offset = Some(-250.0);
        assert_eq!(
            lyrics_to_lrc_or_plain_text(&[synced]).as_deref(),
            Some("[offset:-250]\n[00:00.00]a\n[01:01.23]b\n[60:00.00]c")
        );
    }

    #[test]
    fn prefers_synced_main() {
        let items = [
            lyric(false, None, &[(0, "plain")]),
            lyric(true, Some("translation"), &[(0, "tr")]),
            lyric(true, Some("main"), &[(1000, "sync")]),
        ];
        assert_eq!(
            lyrics_to_lrc_or_plain_text(&items).as_deref(),
            Some("[00:01.00]sync")
        );
    }

    #[test]
    fn plain_and_empty() {
        let items = [lyric(false, None, &[(0, "a"), (0, "b")])];
        assert_eq!(lyrics_to_lrc_or_plain_text(&items).as_deref(), Some("a\nb"));
        assert_eq!(
            lyrics_to_lrc_or_plain_text(&[lyric(true, None, &[(0, " ")])]),
            None
        );
        assert_eq!(lyrics_to_lrc_or_plain_text(&[]), None);
    }
}
