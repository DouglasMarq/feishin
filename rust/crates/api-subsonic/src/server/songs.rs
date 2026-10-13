use feishin_api::Result;
use feishin_core::model::{Page, Paging, Song, SongListQuery, SongSort};

use super::SubsonicServer;
use super::cached_lists::starred_songs_matching;
use super::counting::count_by_probing;
use super::pages::{client_page, cloned_page, non_empty_search, server_page, wants_favorites_only};
use super::requests::SearchWindow;
use crate::album_list::nonzero_year_filter;
use crate::client::Params;
use crate::sort::sort_songs;
use crate::types::RandomSongs;

const LIST_ALL_SONGS_QUERY: &str = "";

enum SongSource<'query> {
    Search(&'query str),
    Random,
    Genre(&'query str),
    Favorites,
    AlbumsAndArtists,
    All,
}

fn filters_by_ids(query: &SongListQuery) -> bool {
    !query.album_ids.is_empty() || !query.artist_ids.is_empty()
}

fn is_random_listing(query: &SongListQuery) -> bool {
    query.sort == SongSort::Random
        && !wants_favorites_only(query.favorite)
        && !filters_by_ids(query)
}

impl<'query> SongSource<'query> {
    fn for_count(query: &'query SongListQuery) -> Self {
        if let Some(term) = non_empty_search(query.search.as_ref()) {
            return Self::Search(term);
        }
        if let Some(genre) = query.genre_ids.first() {
            return Self::Genre(genre);
        }
        if wants_favorites_only(query.favorite) {
            return Self::Favorites;
        }
        match filters_by_ids(query) {
            true => Self::AlbumsAndArtists,
            false => Self::All,
        }
    }

    fn for_list(query: &'query SongListQuery) -> Self {
        match Self::for_count(query) {
            Self::Search(term) => Self::Search(term),
            _ if is_random_listing(query) => Self::Random,
            source => source,
        }
    }
}

fn page_window(paging: Paging) -> SearchWindow {
    SearchWindow {
        offset: paging.start_index,
        count: paging.limit,
    }
}

impl SubsonicServer {
    pub(super) async fn fetch_song_page(
        &self,
        query: &SongListQuery,
        paging: Paging,
    ) -> Result<Page<Song>> {
        let folder = query.music_folder_id.as_deref();
        let window = page_window(paging);
        match SongSource::for_list(query) {
            SongSource::Search(term) => self.searched_song_page(term, paging, folder).await,
            SongSource::Random => self.random_song_page(query, paging).await,
            SongSource::Genre(genre) => {
                let songs = self.songs_by_genre(genre, window, folder).await?;
                Ok(server_page(self.normalize_songs(songs), paging, None))
            }
            SongSource::Favorites => {
                let starred = self.starred(folder).await?;
                let mut songs = starred_songs_matching(&starred, query);
                sort_songs(&mut songs, query.sort, query.order);
                Ok(cloned_page(client_page(songs, paging)))
            }
            SongSource::AlbumsAndArtists => {
                let songs = self
                    .songs_of_albums_and_artists(&query.album_ids, &query.artist_ids)
                    .await?;
                Ok(client_page(self.normalize_songs(songs), paging))
            }
            SongSource::All => {
                self.searched_song_page(LIST_ALL_SONGS_QUERY, paging, folder)
                    .await
            }
        }
    }

    async fn searched_song_page(
        &self,
        term: &str,
        paging: Paging,
        folder: Option<&str>,
    ) -> Result<Page<Song>> {
        let songs = self.search_songs(term, page_window(paging), folder).await?;
        Ok(server_page(self.normalize_songs(songs), paging, None))
    }

    async fn random_song_page(&self, query: &SongListQuery, paging: Paging) -> Result<Page<Song>> {
        let params = Params::new()
            .add("size", paging.limit)
            .add_optional("genre", query.genre_ids.first())
            .add_optional("fromYear", nonzero_year_filter(query.year_from))
            .add_optional("toYear", nonzero_year_filter(query.year_to))
            .add_optional("musicFolderId", query.music_folder_id.as_deref());
        let response: RandomSongs = self.fetch("getRandomSongs.view", params).await?;
        let songs = response
            .random_songs
            .and_then(|container| container.song)
            .unwrap_or_default();
        Ok(server_page(self.normalize_songs(songs), paging, None))
    }

    pub(super) async fn count_songs(&self, query: &SongListQuery) -> Result<usize> {
        let folder = query.music_folder_id.as_deref();
        match SongSource::for_count(query) {
            SongSource::Search(term) => {
                count_by_probing(|offset, count| async move {
                    let window = SearchWindow { offset, count };
                    Ok(self.search_songs(term, window, folder).await?.len())
                })
                .await
            }
            SongSource::Genre(genre) => {
                count_by_probing(|offset, count| async move {
                    let window = SearchWindow { offset, count };
                    Ok(self.songs_by_genre(genre, window, folder).await?.len())
                })
                .await
            }
            SongSource::Favorites => {
                Ok(starred_songs_matching(&*self.starred(folder).await?, query).len())
            }
            SongSource::AlbumsAndArtists => Ok(self
                .songs_of_albums_and_artists(&query.album_ids, &query.artist_ids)
                .await?
                .len()),
            SongSource::All | SongSource::Random => {
                count_by_probing(|offset, count| async move {
                    let window = SearchWindow { offset, count };
                    Ok(self
                        .search_songs(LIST_ALL_SONGS_QUERY, window, folder)
                        .await?
                        .len())
                })
                .await
            }
        }
    }
}
