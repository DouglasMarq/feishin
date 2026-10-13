use feishin_core::model::{Album, Artist, Paging, SearchResults, Song};

use super::cards::{CardKind, album_card, artist_card};
use super::section::{Applied, Section};
use crate::ui::tracks::track_row;
use crate::ui::{SearchCard, SearchKind, TrackRow};

pub enum More {
    Songs(Vec<Song>),
    Albums(Vec<Album>),
    Artists(Vec<Artist>),
}

pub struct MoreRequest {
    pub generation: u64,
    pub query: String,
    pub paging: Paging,
}

#[derive(Default)]
pub struct Searches {
    generation: u64,
    query: Option<String>,
    pub pending: bool,
    pub songs: Section<Song>,
    pub albums: Section<Album>,
    pub artists: Section<Artist>,
}

impl Searches {
    pub fn begin(&mut self, query: Option<String>, force: bool) -> Option<u64> {
        if self.query == query && !force {
            return None;
        }
        self.generation += 1;
        self.pending = query.is_some();
        if query.is_none() {
            self.clear_sections();
        }
        self.query = query;
        Some(self.generation)
    }

    fn clear_sections(&mut self) {
        self.songs = Section::default();
        self.albums = Section::default();
        self.artists = Section::default();
    }

    pub fn finish(
        &mut self,
        generation: u64,
        results: Option<SearchResults>,
        limit: usize,
    ) -> bool {
        if generation != self.generation {
            return false;
        }
        self.pending = false;
        if let Some(results) = results {
            self.songs = Section::new(results.songs, limit);
            self.albums = Section::new(results.albums, limit);
            self.artists = Section::new(results.artists, limit);
        }
        true
    }

    pub fn more(&mut self, kind: SearchKind) -> Option<MoreRequest> {
        if self.pending {
            return None;
        }
        let query = self.query.clone()?;
        let paging = match kind {
            SearchKind::Songs => self.songs.next_page(),
            SearchKind::Albums => self.albums.next_page(),
            SearchKind::Artists => self.artists.next_page(),
        }?;
        Some(MoreRequest {
            generation: self.generation,
            query,
            paging,
        })
    }

    pub fn more_failed(&mut self, generation: u64, kind: SearchKind) {
        if generation != self.generation {
            return;
        }
        match kind {
            SearchKind::Songs => self.songs.busy = false,
            SearchKind::Albums => self.albums.busy = false,
            SearchKind::Artists => self.artists.busy = false,
        }
    }

    pub fn apply_more(&mut self, generation: u64, paging: Paging, more: More) -> Option<Applied> {
        (generation == self.generation).then(|| match more {
            More::Songs(items) => self.songs.apply(paging, items),
            More::Albums(items) => self.albums.apply(paging, items),
            More::Artists(items) => self.artists.apply(paging, items),
        })
    }

    pub fn song(&self, index: i32) -> Option<&Song> {
        self.songs.items.get(usize::try_from(index).ok()?)
    }

    pub fn song_row(&self, index: usize) -> Option<TrackRow> {
        self.songs
            .items
            .get(index)
            .map(|song| track_row(song, index))
    }

    pub fn cards_from(&self, kind: CardKind, start: usize) -> Vec<SearchCard> {
        match kind {
            CardKind::Albums => self
                .albums
                .items
                .iter()
                .skip(start)
                .map(album_card)
                .collect(),
            CardKind::Artists => self
                .artists
                .items
                .iter()
                .skip(start)
                .map(artist_card)
                .collect(),
        }
    }

    pub fn cover_source(&self, kind: CardKind, index: usize) -> Option<(String, String)> {
        let (id, cover_id) = match kind {
            CardKind::Albums => {
                let album = self.albums.items.get(index)?;
                (&album.id, &album.cover_art_id)
            }
            CardKind::Artists => {
                let artist = self.artists.items.get(index)?;
                (&artist.id, &artist.cover_art_id)
            }
        };
        Some((id.clone(), cover_id.clone()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::search::query::SEARCH_LIMIT;
    use crate::ui::search::test_support::numbered_songs;

    fn results_with_songs(count: usize) -> SearchResults {
        SearchResults {
            songs: numbered_songs(count),
            albums: Vec::new(),
            artists: Vec::new(),
        }
    }

    fn requested_more(searches: &mut Searches) -> MoreRequest {
        searches.more(SearchKind::Songs).unwrap_or(MoreRequest {
            generation: 0,
            query: String::new(),
            paging: Paging {
                start_index: 0,
                limit: 0,
            },
        })
    }

    #[test]
    fn stale_responses_are_dropped() {
        let mut searches = Searches::default();
        let first = searches.begin(Some("riv".into()), false);
        let second = searches.begin(Some("river".into()), false);
        assert_eq!(searches.begin(Some("river".into()), false), None);
        assert!(searches.pending);
        assert!(searches.more(SearchKind::Songs).is_none());

        assert!(!searches.finish(
            first.unwrap_or_default(),
            Some(results_with_songs(1)),
            SEARCH_LIMIT
        ));
        assert!(searches.songs.items.is_empty());
        assert!(searches.finish(
            second.unwrap_or_default(),
            Some(results_with_songs(SEARCH_LIMIT)),
            SEARCH_LIMIT
        ));
        assert!(!searches.pending);
        assert_eq!(searches.song(0).map(|song| song.id.as_str()), Some("s0"));

        let request = requested_more(&mut searches);
        assert_eq!(request.query, "river");
        searches.begin(Some("rivers".into()), false);
        assert_eq!(
            searches.apply_more(
                request.generation,
                request.paging,
                More::Songs(numbered_songs(5))
            ),
            None
        );

        searches.begin(None, false);
        assert!(!searches.pending);
        assert!(searches.songs.items.is_empty());
    }
}
