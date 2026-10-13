use crate::watch_url;

const MUSIC_ORIGIN: &str = "https://music.youtube.com";
const PLAYLIST_BROWSE_PREFIX: &str = "VL";
const ALBUM_BROWSE_PREFIX: &str = "MPRE";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusicSong {
    pub video_id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub thumbnail_url: Option<String>,
}

impl MusicSong {
    pub fn web_url(&self) -> String {
        watch_url(&self.video_id)
    }

    pub fn artist_names(&self) -> String {
        self.artists.join(", ")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CollectionKind {
    Album,
    Playlist,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusicCollection {
    pub browse_id: String,
    pub kind: CollectionKind,
    pub title: String,
    pub subtitle: String,
    pub thumbnail_url: Option<String>,
}

impl MusicCollection {
    pub fn web_url(&self) -> String {
        collection_url(&self.browse_id)
    }
}

pub fn collection_url(browse_id: &str) -> String {
    match browse_id.strip_prefix(PLAYLIST_BROWSE_PREFIX) {
        Some(playlist_id) => format!("{MUSIC_ORIGIN}/playlist?list={playlist_id}"),
        None => format!("{MUSIC_ORIGIN}/browse/{browse_id}"),
    }
}

pub fn collection_kind(browse_id: &str) -> CollectionKind {
    match browse_id.starts_with(ALBUM_BROWSE_PREFIX) {
        true => CollectionKind::Album,
        false => CollectionKind::Playlist,
    }
}

pub fn is_browse_id(candidate: &str) -> bool {
    !candidate.is_empty()
        && candidate.len() <= 128
        && candidate
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShelfItem {
    Song(MusicSong),
    Collection(MusicCollection),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusicShelf {
    pub title: String,
    pub items: Vec<ShelfItem>,
}

impl MusicShelf {
    pub fn songs(&self) -> impl Iterator<Item = &MusicSong> {
        self.items.iter().filter_map(|item| match item {
            ShelfItem::Song(song) => Some(song),
            ShelfItem::Collection(_) => None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoodCategory {
    pub title: String,
    pub params: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionPage {
    pub title: String,
    pub subtitle: String,
    pub thumbnail_url: Option<String>,
    pub songs: Vec<MusicSong>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collections_link_to_their_youtube_music_page() {
        assert_eq!(
            collection_url("VLPLabc"),
            "https://music.youtube.com/playlist?list=PLabc"
        );
        assert_eq!(
            collection_url("MPREb_x"),
            "https://music.youtube.com/browse/MPREb_x"
        );
    }

    #[test]
    fn album_ids_start_with_mpre() {
        assert_eq!(collection_kind("MPREb_x"), CollectionKind::Album);
        assert_eq!(collection_kind("VLPLx"), CollectionKind::Playlist);
    }

    #[test]
    fn browse_ids_are_url_safe() {
        assert!(is_browse_id("MPREb_g3H4u5nW8xD"));
        assert!(!is_browse_id(""));
        assert!(!is_browse_id("VL abc"));
        assert!(!is_browse_id("../x"));
    }
}
