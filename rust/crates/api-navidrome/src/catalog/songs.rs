use feishin_api::{ApiError, Result};
use feishin_core::model::{Page, Paging, Song, SongListQuery};

use crate::NavidromeServer;
use crate::catalog::SUBSONIC_MAX_ITEMS;
use crate::catalog::paging::subsonic_page;
use crate::features::Features;
use crate::http::missing_session_error;
use crate::native::{NativeSong, normalize_song};
use crate::query::{self, QueryParams, end_index_or_all};
use crate::subsonic::{self, SubsonicSong};

const ALBUM_IDS_BATCH_SIZE: usize = 500;
const COUNT_PAGING: Paging = Paging {
    start_index: 0,
    limit: 1,
};

impl NavidromeServer {
    pub(crate) fn songs_from_native(&self, songs: Vec<NativeSong>) -> Vec<Song> {
        songs
            .into_iter()
            .map(|song| normalize_song(song, self.server_id()))
            .collect()
    }

    pub(crate) fn songs_from_subsonic(&self, songs: Vec<SubsonicSong>) -> Vec<Song> {
        songs
            .into_iter()
            .map(|song| subsonic::normalize_song(song, self.server_id()))
            .collect()
    }

    pub(crate) async fn list_songs(
        &self,
        query: &SongListQuery,
        paging: Paging,
    ) -> Result<Page<Song>> {
        let features = self.features().await;
        self.native_or_subsonic(
            self.native_song_page(query, paging, &features),
            self.subsonic_song_page(query, paging),
        )
        .await
    }

    pub(crate) async fn count_songs(&self, query: &SongListQuery) -> Result<usize> {
        let features = self.features().await;
        let page = self
            .native_song_page(query, COUNT_PAGING, &features)
            .await?;
        Ok(page.total_count.unwrap_or(0))
    }

    pub(crate) async fn fetch_song(&self, id: &str) -> Result<Song> {
        self.native_or_subsonic(self.native_song(id), self.subsonic_song(id))
            .await
    }

    pub(crate) async fn fetch_random_songs(
        &self,
        count: usize,
        genre: Option<&str>,
    ) -> Result<Vec<Song>> {
        let mut params: QueryParams = vec![("size", count.to_string())];
        params.extend(genre.map(|genre| ("genre", genre.to_owned())));
        let response: subsonic::RandomSongs = self.subsonic("getRandomSongs", &params).await?;
        Ok(self.songs_from_subsonic(response.random_songs.unwrap_or_default().song))
    }

    pub(crate) async fn fetch_similar_songs(
        &self,
        song_id: &str,
        count: usize,
    ) -> Result<Vec<Song>> {
        let params: QueryParams = vec![("id", song_id.to_owned()), ("count", count.to_string())];
        let response: subsonic::SimilarSongs = self.subsonic("getSimilarSongs", &params).await?;
        let songs = response
            .similar_songs
            .unwrap_or_default()
            .song
            .into_iter()
            .filter(|song| song.id != song_id)
            .collect();
        Ok(self.songs_from_subsonic(songs))
    }

    async fn native_song(&self, id: &str) -> Result<Song> {
        let (song, _): (NativeSong, _) = self.native_get(&format!("song/{id}"), &[]).await?;
        Ok(normalize_song(song, self.server_id()))
    }

    async fn subsonic_song(&self, id: &str) -> Result<Song> {
        let response: subsonic::SongResponse =
            self.subsonic("getSong", &[("id", id.to_owned())]).await?;
        let song = response.song.ok_or(ApiError::NotFound)?;
        Ok(subsonic::normalize_song(song, self.server_id()))
    }

    async fn native_song_page(
        &self,
        query: &SongListQuery,
        paging: Paging,
        features: &Features,
    ) -> Result<Page<Song>> {
        let start = paging.start_index;
        let end = end_index_or_all(start, paging.limit);
        let mut items = Vec::new();
        let mut total: Option<usize> = None;
        for batch in album_id_batches(&query.album_ids) {
            let params = query::song_list(query, start, end, batch, features);
            let (songs, count): (Vec<NativeSong>, _) = self.native_get("song", &params).await?;
            items.extend(self.songs_from_native(songs));
            total = count.map(|count| total.unwrap_or(0) + count).or(total);
        }
        Ok(Page {
            items,
            start_index: start,
            total_count: total,
        })
    }

    async fn subsonic_song_page(
        &self,
        query: &SongListQuery,
        paging: Paging,
    ) -> Result<Page<Song>> {
        if has_native_only_song_filters(query) {
            return Err(missing_session_error());
        }
        let params = subsonic_song_search_params(query, paging);
        let result: subsonic::Search3 = self.subsonic("search3", &params).await?;
        let songs = self.songs_from_subsonic(result.search_result3.unwrap_or_default().song);
        Ok(subsonic_page(songs, paging.start_index))
    }
}

fn album_id_batches(album_ids: &[String]) -> Vec<&[String]> {
    if album_ids.len() > ALBUM_IDS_BATCH_SIZE {
        return album_ids.chunks(ALBUM_IDS_BATCH_SIZE).collect();
    }
    vec![album_ids]
}

fn has_native_only_song_filters(query: &SongListQuery) -> bool {
    query.favorite.is_some()
        || !query.genre_ids.is_empty()
        || !query.artist_ids.is_empty()
        || !query.album_ids.is_empty()
        || query.year_from.is_some()
        || query.year_to.is_some()
}

fn subsonic_song_search_params(query: &SongListQuery, paging: Paging) -> QueryParams {
    let limit = match paging.limit {
        0 => SUBSONIC_MAX_ITEMS,
        limit => limit,
    };
    let mut params: QueryParams = vec![
        ("query", query.search.clone().unwrap_or_default()),
        ("songCount", limit.to_string()),
        ("songOffset", paging.start_index.to_string()),
        ("albumCount", "0".into()),
        ("artistCount", "0".into()),
    ];
    params.extend(
        query
            .music_folder_id
            .clone()
            .map(|folder| ("musicFolderId", folder)),
    );
    params
}
