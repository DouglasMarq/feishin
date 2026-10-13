use serde::Deserialize;

use crate::lenient::{optional_f64, optional_string, optional_u64, string, truthy};

const MAIN_LYRIC_KIND: &str = "main";
const MILLISECONDS_PER_MINUTE: u64 = 60_000;
const MILLISECONDS_PER_SECOND: u64 = 1000;
const MILLISECONDS_PER_CENTISECOND: u64 = 10;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct LyricLine {
    #[serde(deserialize_with = "optional_u64")]
    pub start: Option<u64>,
    #[serde(deserialize_with = "string")]
    pub value: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct StructuredLyric {
    #[serde(deserialize_with = "optional_string")]
    pub kind: Option<String>,
    #[serde(deserialize_with = "truthy")]
    pub synced: bool,
    #[serde(deserialize_with = "optional_f64")]
    pub offset: Option<f64>,
    pub line: Vec<LyricLine>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct LyricsListBody {
    pub structured_lyrics: Vec<StructuredLyric>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct LyricsList {
    pub lyrics_list: Option<LyricsListBody>,
}

pub(crate) fn lyrics_text(lyrics: Vec<StructuredLyric>) -> Option<String> {
    let lyric = pick_displayed_lyric(lyrics)?;
    if lyric.synced {
        return Some(render_synced(lyric));
    }
    Some(render_plain(lyric))
}

fn pick_displayed_lyric(lyrics: Vec<StructuredLyric>) -> Option<StructuredLyric> {
    let index = lyrics
        .iter()
        .position(|lyric| is_main(lyric) && lyric.synced && has_lines(lyric))
        .or_else(|| {
            lyrics
                .iter()
                .position(|lyric| is_main(lyric) && has_lines(lyric))
        })
        .or_else(|| lyrics.iter().position(has_lines))?;
    lyrics.into_iter().nth(index)
}

fn is_main(lyric: &StructuredLyric) -> bool {
    lyric
        .kind
        .as_deref()
        .is_none_or(|kind| kind == MAIN_LYRIC_KIND)
}

fn has_lines(lyric: &StructuredLyric) -> bool {
    !lyric.line.is_empty()
}

fn render_plain(lyric: StructuredLyric) -> String {
    let lines: Vec<String> = lyric.line.into_iter().map(|line| line.value).collect();
    lines.join("\n")
}

fn render_synced(lyric: StructuredLyric) -> String {
    let offset_tag = lyric
        .offset
        .filter(|offset| *offset != 0.0)
        .map(|offset| format!("[offset:{}]", offset.round() as i64));
    let timed_lines = lyric.line.into_iter().map(|line| {
        let timestamp = lrc_timestamp(line.start.unwrap_or(0));
        format!("{timestamp}{}", line.value)
    });
    let rendered: Vec<String> = offset_tag.into_iter().chain(timed_lines).collect();
    rendered.join("\n")
}

fn lrc_timestamp(start_ms: u64) -> String {
    let minutes = start_ms / MILLISECONDS_PER_MINUTE;
    let seconds = (start_ms % MILLISECONDS_PER_MINUTE) / MILLISECONDS_PER_SECOND;
    let centiseconds = (start_ms % MILLISECONDS_PER_SECOND) / MILLISECONDS_PER_CENTISECOND;
    format!("[{minutes:02}:{seconds:02}.{centiseconds:02}]")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn synced_lyrics_render_as_lrc() {
        let list: LyricsList = serde_json::from_value(json!({
            "lyricsList": {"structuredLyrics": [
                {"lang": "eng", "synced": false, "line": [{"value": "plain"}]},
                {"lang": "eng", "synced": true, "offset": 100, "line": [
                    {"start": 0, "value": "first"},
                    {"start": 61_230, "value": "second"}
                ]}
            ]}
        }))
        .expect("fixture");
        let text = lyrics_text(list.lyrics_list.unwrap_or_default().structured_lyrics);
        assert_eq!(
            text.as_deref(),
            Some("[offset:100]\n[00:00.00]first\n[01:01.23]second")
        );
    }

    #[test]
    fn unsynced_lyrics_join_lines() {
        let lyric = StructuredLyric {
            kind: None,
            synced: false,
            offset: None,
            line: vec![
                LyricLine {
                    start: None,
                    value: "a".into(),
                },
                LyricLine {
                    start: None,
                    value: "b".into(),
                },
            ],
        };
        assert_eq!(lyrics_text(vec![lyric]).as_deref(), Some("a\nb"));
        assert_eq!(lyrics_text(Vec::new()), None);
    }
}
