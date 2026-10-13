use std::collections::HashSet;

use feishin_api::{ApiError, Result};
use feishin_core::model::{Page, Paging, Song, SongListQuery, SongSort, SortOrder};

use super::{JellyfinServer, MAX_IDS_PER_QUERY_STRING};
use crate::dto;
use crate::item_fields::{SONG_FIELDS, SONG_ITEM_TYPE};
use crate::normalize;
use crate::query_params::{QueryParams, comma_separated_ids, comma_separated_years};
use crate::sorting::{song_sort, sort_order};

struct FetchedSongs {
    items: Vec<dto::Song>,
    total: usize,
}

impl FetchedSongs {
    fn counted(items: Vec<dto::Song>) -> Self {
        let total = items.len();
        Self { items, total }
    }
}

struct SongWindow {
    start: usize,
    limit: Option<usize>,
}

impl JellyfinServer {
    pub(super) fn normalize_songs(&self, items: Vec<dto::Song>) -> Vec<Song> {
        items
            .into_iter()
            .map(|item| normalize::song(item, self.server_id()))
            .collect()
    }

    pub(super) async fn song_page(
        &self,
        query: &SongListQuery,
        paging: Paging,
    ) -> Result<Page<Song>> {
        let window = SongWindow {
            start: paging.start_index,
            limit: Some(paging.limit),
        };
        let fetched = self.fetch_songs(query, &window).await?;
        Ok(Page {
            items: self.normalize_songs(fetched.items),
            start_index: paging.start_index,
            total_count: Some(fetched.total),
        })
    }

    pub(super) async fn count_songs(&self, query: &SongListQuery) -> Result<usize> {
        let window = SongWindow {
            start: 0,
            limit: query.album_ids.is_empty().then_some(1),
        };
        Ok(self.fetch_songs(query, &window).await?.total)
    }

    pub(super) async fn song_by_id(&self, id: &str) -> Result<Song> {
        let item: dto::Song = self
            .get(&self.item_path(id), &QueryParams::default())
            .await?;
        Ok(normalize::song(item, self.server_id()))
    }

    pub(super) async fn random_song_list(
        &self,
        count: usize,
        genre: Option<&str>,
    ) -> Result<Vec<Song>> {
        let params = QueryParams::default()
            .fields(SONG_FIELDS)
            .set_optional("GenreIds", genre)
            .set("IncludeItemTypes", SONG_ITEM_TYPE)
            .set("Limit", count)
            .set("Recursive", true)
            .set("SortBy", song_sort(SongSort::Random))
            .set("SortOrder", sort_order(SortOrder::Asc))
            .set("StartIndex", 0);
        let list: dto::ItemList<dto::Song> = self.get(&self.items_path(), &params).await?;
        Ok(self.normalize_songs(list.items))
    }

    pub(super) async fn similar_song_list(&self, song_id: &str, count: usize) -> Result<Vec<Song>> {
        let similar_path = format!("items/{song_id}/similar");
        match self.mix(&similar_path, song_id, count).await {
            Ok(songs) if !songs.is_empty() => Ok(songs),
            Ok(_no_similar_songs) => self.instant_mix(song_id, count).await,
            Err(error) => {
                tracing::debug!(%error, song_id, "similar songs unavailable, falling back to instant mix");
                self.instant_mix(song_id, count).await
            }
        }
    }

    async fn instant_mix(&self, song_id: &str, count: usize) -> Result<Vec<Song>> {
        let instant_mix_path = format!("items/{song_id}/InstantMix");
        self.mix(&instant_mix_path, song_id, count).await
    }

    pub(super) async fn song_lyrics(&self, song: &Song) -> Result<Option<String>> {
        let lyrics_path = format!("audio/{}/Lyrics", song.id);
        match self
            .get::<dto::Lyrics>(&lyrics_path, &QueryParams::default())
            .await
        {
            Ok(lyrics) => Ok(normalize::lyrics(lyrics)),
            Err(ApiError::NotFound) => Ok(None),
            Err(error) => Err(error),
        }
    }

    async fn mix(&self, path: &str, seed_song_id: &str, count: usize) -> Result<Vec<Song>> {
        let params = QueryParams::default()
            .fields(SONG_FIELDS)
            .set("Limit", count)
            .set("UserId", &self.user_id);
        let list: dto::ItemList<dto::Song> = self.get(path, &params).await?;
        let other_songs = list
            .items
            .into_iter()
            .filter(|item| item.id != seed_song_id)
            .collect();
        Ok(self.normalize_songs(other_songs))
    }

    async fn fetch_songs(
        &self,
        query: &SongListQuery,
        window: &SongWindow,
    ) -> Result<FetchedSongs> {
        if query.album_ids.len() > MAX_IDS_PER_QUERY_STRING {
            return self.fetch_songs_in_album_batches(query, window).await;
        }
        self.fetch_songs_in_one_request(query, window).await
    }

    async fn fetch_songs_in_album_batches(
        &self,
        query: &SongListQuery,
        window: &SongWindow,
    ) -> Result<FetchedSongs> {
        let path = self.items_path();
        let mut items = Vec::new();
        for album_batch in query.album_ids.chunks(MAX_IDS_PER_QUERY_STRING) {
            let params = song_params(query, album_batch, window);
            let list: dto::ItemList<dto::Song> = self.get(&path, &params).await?;
            items.extend(list.items);
        }
        Ok(FetchedSongs::counted(items))
    }

    async fn fetch_songs_in_one_request(
        &self,
        query: &SongListQuery,
        window: &SongWindow,
    ) -> Result<FetchedSongs> {
        let params = song_params(query, &query.album_ids, window);
        let list: dto::ItemList<dto::Song> = self.get(&self.items_path(), &params).await?;
        if query.album_ids.is_empty() {
            let total = list.total_record_count.unwrap_or(list.items.len());
            return Ok(FetchedSongs {
                items: list.items,
                total,
            });
        }
        Ok(FetchedSongs::counted(keep_songs_of_albums(
            list.items,
            &query.album_ids,
        )))
    }
}

fn keep_songs_of_albums(items: Vec<dto::Song>, album_ids: &[String]) -> Vec<dto::Song> {
    let wanted_album_ids: HashSet<&str> = album_ids.iter().map(String::as_str).collect();
    items
        .into_iter()
        .filter(|item| {
            item.album_id
                .as_deref()
                .is_some_and(|album_id| wanted_album_ids.contains(album_id))
        })
        .collect()
}

fn song_params(query: &SongListQuery, album_ids: &[String], window: &SongWindow) -> QueryParams {
    QueryParams::default()
        .set_optional("AlbumIds", comma_separated_ids(album_ids))
        .set_optional("ArtistIds", comma_separated_ids(&query.artist_ids))
        .fields(SONG_FIELDS)
        .set_optional("GenreIds", comma_separated_ids(&query.genre_ids))
        .set("IncludeItemTypes", SONG_ITEM_TYPE)
        .set_optional("IsFavorite", query.favorite)
        .set_optional("Limit", window.limit)
        .set_optional("ParentId", query.music_folder_id.as_deref())
        .set("Recursive", true)
        .set_optional("SearchTerm", query.search.as_deref())
        .set("SortBy", song_sort(query.sort))
        .set("SortOrder", sort_order(query.order))
        .set("StartIndex", window.start)
        .set_optional(
            "Years",
            comma_separated_years(query.year_from, query.year_to),
        )
}
