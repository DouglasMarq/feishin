//! OpenSubsonic structured lyrics to LRC / plain text.

use crate::types::StructuredLyric;

fn is_main(lyric: &StructuredLyric) -> bool {
    lyric.kind.as_deref().is_none_or(|k| k == "main")
}

fn timestamp(ms: u64) -> String {
    let centis = ms / 10;
    format!(
        "[{:02}:{:02}.{:02}]",
        centis / 6000,
        (centis / 100) % 60,
        centis % 100
    )
}

/// Picks synced main lyrics first, then any main lyrics, then whatever is first.
pub fn to_text(lyrics: &[StructuredLyric]) -> Option<String> {
    let lyric = lyrics
        .iter()
        .find(|l| l.synced && is_main(l))
        .or_else(|| lyrics.iter().find(|l| is_main(l)))
        .or_else(|| lyrics.first())?;

    if lyric.line.iter().all(|l| l.value.trim().is_empty()) {
        return None;
    }

    if !lyric.synced {
        let lines: Vec<&str> = lyric.line.iter().map(|l| l.value.as_str()).collect();
        return Some(lines.join("\n"));
    }

    let mut out = Vec::with_capacity(lyric.line.len() + 1);
    if let Some(offset) = lyric.offset.filter(|o| o.is_finite() && *o != 0.0) {
        out.push(format!("[offset:{}]", offset.round() as i64));
    }
    for line in &lyric.line {
        let start = line
            .start
            .filter(|s| s.is_finite() && *s > 0.0)
            .map_or(0, |s| s.round() as u64);
        out.push(format!("{}{}", timestamp(start), line.value));
    }
    Some(out.join("\n"))
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
        let mut l = lyric(true, None, &[(0, "a"), (61_234, "b"), (3_600_000, "c")]);
        l.offset = Some(-250.0);
        assert_eq!(
            to_text(&[l]).as_deref(),
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
        assert_eq!(to_text(&items).as_deref(), Some("[00:01.00]sync"));
    }

    #[test]
    fn plain_and_empty() {
        let items = [lyric(false, None, &[(0, "a"), (0, "b")])];
        assert_eq!(to_text(&items).as_deref(), Some("a\nb"));
        assert_eq!(to_text(&[lyric(true, None, &[(0, " ")])]), None);
        assert_eq!(to_text(&[]), None);
    }
}
