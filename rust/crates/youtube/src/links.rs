use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};

const WATCH_PREFIX: &str = "https://music.youtube.com/watch?v=";
const SEARCH_PREFIX: &str = "https://music.youtube.com/search?q=";
const SONGS_SECTION: &str = "#songs";

pub fn is_video_id(candidate: &str) -> bool {
    !candidate.is_empty()
        && candidate.len() <= 64
        && candidate
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

pub fn watch_url(video_id: &str) -> String {
    format!("{WATCH_PREFIX}{video_id}")
}

pub fn thumbnail_url(video_id: &str) -> String {
    format!("https://i.ytimg.com/vi/{video_id}/hqdefault.jpg")
}

pub(crate) fn songs_search_url(query: &str) -> String {
    let encoded = utf8_percent_encode(query.trim(), NON_ALPHANUMERIC);
    format!("{SEARCH_PREFIX}{encoded}{SONGS_SECTION}")
}
