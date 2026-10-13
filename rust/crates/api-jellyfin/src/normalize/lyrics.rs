use super::ticks_to_ms;
use crate::dto;

const MS_PER_MINUTE: u64 = 60_000;
const MS_PER_SECOND: u64 = 1000;
const MS_PER_CENTISECOND: u64 = 10;

pub fn lyrics(lyrics: dto::Lyrics) -> Option<String> {
    let lines = lyrics.lyrics;
    let render_line: fn(dto::LyricLine) -> String = match lines.first()?.start {
        Some(_synced_start) => lrc_line,
        None => plain_line,
    };
    let rendered_lines: Vec<String> = lines.into_iter().map(render_line).collect();
    Some(rendered_lines.join("\n"))
}

fn plain_line(line: dto::LyricLine) -> String {
    line.text
}

fn lrc_line(line: dto::LyricLine) -> String {
    format!("{}{}", lrc_timestamp(ticks_to_ms(line.start)), line.text)
}

fn lrc_timestamp(position_ms: u64) -> String {
    let minutes = position_ms / MS_PER_MINUTE;
    let seconds = (position_ms % MS_PER_MINUTE) / MS_PER_SECOND;
    let centiseconds = (position_ms % MS_PER_SECOND) / MS_PER_CENTISECOND;
    format!("[{minutes:02}:{seconds:02}.{centiseconds:02}]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synced_lyrics_become_lrc() {
        let synced = dto::Lyrics {
            lyrics: vec![
                dto::LyricLine {
                    start: Some(0),
                    text: "one".into(),
                },
                dto::LyricLine {
                    start: Some(615_300_000),
                    text: "two".into(),
                },
            ],
        };
        assert_eq!(lyrics(synced).unwrap(), "[00:00.00]one\n[01:01.53]two");
    }

    #[test]
    fn unsynced_lyrics_are_plain() {
        let unsynced = dto::Lyrics {
            lyrics: vec![
                dto::LyricLine {
                    start: None,
                    text: "a".into(),
                },
                dto::LyricLine {
                    start: None,
                    text: "b".into(),
                },
            ],
        };
        assert_eq!(lyrics(unsynced).unwrap(), "a\nb");
        assert!(lyrics(dto::Lyrics::default()).is_none());
    }
}
