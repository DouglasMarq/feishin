use feishin_youtube::music::{
    CollectionKind, CollectionPage, MusicSong, collection_kind, collection_url, is_browse_id,
};

pub const SEARCH_PREFIX: &str = "search:";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageTarget {
    Collection {
        browse_id: String,
        kind: CollectionKind,
    },
    Search(String),
}

impl PageTarget {
    pub fn parse(detail_id: &str) -> Option<Self> {
        if let Some(query) = detail_id.strip_prefix(SEARCH_PREFIX) {
            let query = query.trim();
            return (!query.is_empty()).then(|| Self::Search(query.to_owned()));
        }
        is_browse_id(detail_id).then(|| Self::Collection {
            browse_id: detail_id.to_owned(),
            kind: collection_kind(detail_id),
        })
    }

    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Collection {
                kind: CollectionKind::Album,
                ..
            } => "ALBUM",
            Self::Collection {
                kind: CollectionKind::Playlist,
                ..
            } => "PLAYLIST",
            Self::Search(_) => "SEARCH RESULTS",
        }
    }

    pub fn web_url(&self) -> Option<String> {
        match self {
            Self::Collection { browse_id, .. } => Some(collection_url(browse_id)),
            Self::Search(_) => None,
        }
    }
}

pub fn search_detail_id(query: &str) -> Option<String> {
    let query = query.trim();
    (!query.is_empty()).then(|| format!("{SEARCH_PREFIX}{query}"))
}

#[derive(Default)]
pub struct Page {
    pub target: Option<PageTarget>,
    pub generation: u64,
    pub loading: bool,
    pub error: Option<String>,
    pub title: String,
    pub subtitle: String,
    pub thumbnail_url: Option<String>,
    pub songs: Vec<MusicSong>,
}

impl Page {
    pub fn open(&mut self, target: PageTarget) -> Option<u64> {
        let already_shown = self.target.as_ref() == Some(&target) && self.error.is_none();
        if already_shown {
            return None;
        }
        self.generation += 1;
        self.loading = true;
        self.error = None;
        self.title = match &target {
            PageTarget::Search(query) => format!("\"{query}\""),
            PageTarget::Collection { .. } => String::new(),
        };
        self.subtitle.clear();
        self.thumbnail_url = None;
        self.songs.clear();
        self.target = Some(target);
        Some(self.generation)
    }

    pub fn show_collection(
        &mut self,
        generation: u64,
        result: Result<CollectionPage, String>,
    ) -> bool {
        if !self.finish(generation) {
            return false;
        }
        match result {
            Ok(page) => {
                self.title = page.title;
                self.subtitle = page.subtitle;
                self.thumbnail_url = page.thumbnail_url;
                self.songs = page.songs;
            }
            Err(error) => self.error = Some(error),
        }
        true
    }

    pub fn show_search(&mut self, generation: u64, result: Result<Vec<MusicSong>, String>) -> bool {
        if !self.finish(generation) {
            return false;
        }
        match result {
            Ok(songs) => {
                self.subtitle = tr_plural!(
                    "{n} song on YouTube Music",
                    "{n} songs on YouTube Music",
                    songs.len()
                );
                self.songs = songs;
            }
            Err(error) => self.error = Some(error),
        }
        true
    }

    fn finish(&mut self, generation: u64) -> bool {
        let current = generation == self.generation;
        if current {
            self.loading = false;
        }
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn album_target() -> PageTarget {
        PageTarget::parse("MPREb_x").unwrap()
    }

    #[test]
    fn detail_ids_open_albums_playlists_or_searches() {
        assert_eq!(album_target().kind_label(), "ALBUM");
        assert_eq!(
            PageTarget::parse("VLPLx").map(|target| target.kind_label()),
            Some("PLAYLIST")
        );
        assert_eq!(
            PageTarget::parse("search: get lucky "),
            Some(PageTarget::Search("get lucky".into()))
        );
        assert_eq!(PageTarget::parse("search:   "), None);
        assert_eq!(PageTarget::parse("not an id"), None);
        assert_eq!(search_detail_id("  hey "), Some("search:hey".into()));
        assert_eq!(search_detail_id(" "), None);
        assert_eq!(
            album_target().web_url().as_deref(),
            Some("https://music.youtube.com/browse/MPREb_x")
        );
    }

    #[test]
    fn stale_answers_are_ignored_and_failed_pages_reload() {
        let mut page = Page::default();
        let first = page.open(album_target()).unwrap();
        assert_eq!(page.open(album_target()), None, "already loading");
        let search = page.open(PageTarget::Search("x".into())).unwrap();
        assert!(!page.show_collection(first, Err("late".into())));
        assert!(page.loading);
        assert!(page.show_search(search, Err("offline".into())));
        assert_eq!(page.error.as_deref(), Some("offline"));
        assert!(
            page.open(PageTarget::Search("x".into())).is_some(),
            "retry after a failure"
        );
    }
}
