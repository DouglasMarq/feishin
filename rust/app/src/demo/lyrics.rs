use super::hashing::{pick_word, scramble};
use super::vocabulary::{ADJECTIVES, NOUNS};

const LINE_SPACING_MS: u64 = 4_000;
const FIRST_LINE_MS: u64 = 2_000;

#[derive(Clone, Copy)]
enum LyricsFormat {
    Plain,
    Synced,
}

impl LyricsFormat {
    fn for_song(index: usize) -> Self {
        match index.is_multiple_of(7) {
            true => Self::Plain,
            false => Self::Synced,
        }
    }

    fn render_line(self, line: u64, text: String) -> String {
        match self {
            Self::Plain => text,
            Self::Synced => format!(
                "{}{text}",
                lrc_timestamp(FIRST_LINE_MS + line * LINE_SPACING_MS)
            ),
        }
    }
}

pub(super) fn demo_lyrics(index: usize, duration_ms: u64) -> String {
    let seed = scramble(index as u64 ^ 0x1e7);
    let line_count = (duration_ms / LINE_SPACING_MS).clamp(4, 90);
    let format = LyricsFormat::for_song(index);
    (0..line_count)
        .map(|line| format.render_line(line, lyric_text(seed, line)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn lyric_text(seed: u64, line: u64) -> String {
    let hash = scramble(seed ^ line);
    format!(
        "{} {} in the {}",
        pick_word(&ADJECTIVES, hash),
        pick_word(&NOUNS, hash >> 16).to_lowercase(),
        pick_word(&NOUNS, hash >> 32).to_lowercase()
    )
}

fn lrc_timestamp(offset_ms: u64) -> String {
    format!(
        "[{:02}:{:02}.{:02}]",
        offset_ms / 60_000,
        offset_ms / 1000 % 60,
        offset_ms % 1000 / 10
    )
}
