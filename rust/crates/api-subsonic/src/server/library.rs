use feishin_api::{ApiError, Result};
use feishin_core::model::{Genre, MusicFolder, SearchResults, Song};

use super::SubsonicServer;
use super::requests::{SearchRequest, SearchWindow};
use crate::client::Params;
use crate::lyrics::lyrics_to_lrc_or_plain_text;
use crate::normalize;
use crate::types::{Genres, GetSong, LyricsList, MusicFolders, RandomSongs, SimilarSongs};

impl SubsonicServer {
    pub(super) async fn fetch_song(&self, id: &str) -> Result<Song> {
        let response: GetSong = self
            .fetch("getSong.view", Params::new().add("id", id))
            .await?;
        Ok(normalize::song(response.song, self.server_id()))
    }

    pub(super) async fn fetch_random_songs(
        &self,
        count: usize,
        genre: Option<&str>,
    ) -> Result<Vec<Song>> {
        let params = Params::new().add("size", count).opt("genre", genre);
        let response: RandomSongs = self.fetch("getRandomSongs.view", params).await?;
        let songs = response
            .random_songs
            .and_then(|container| container.song)
            .unwrap_or_default();
        Ok(self.normalize_songs(songs))
    }

    pub(super) async fn fetch_similar_songs(
        &self,
        song_id: &str,
        count: usize,
    ) -> Result<Vec<Song>> {
        let params = Params::new().add("id", song_id).add("count", count);
        let response: SimilarSongs = self.fetch("getSimilarSongs.view", params).await?;
        let songs = response
            .similar_songs
            .and_then(|container| container.song)
            .unwrap_or_default();
        let mut songs = self.normalize_songs(songs);
        songs.retain(|song| song.id != song_id);
        Ok(songs)
    }

    pub(super) async fn fetch_genres(&self) -> Result<Vec<Genre>> {
        let response: Genres = self.fetch("getGenres.view", Params::new()).await?;
        let mut genres: Vec<Genre> = response
            .genres
            .and_then(|container| container.genre)
            .unwrap_or_default()
            .into_iter()
            .map(normalize::genre)
            .collect();
        genres.sort_by_cached_key(|genre| genre.name.to_lowercase());
        Ok(genres)
    }

    pub(super) async fn fetch_music_folders(&self) -> Result<Vec<MusicFolder>> {
        let response: MusicFolders = self.fetch("getMusicFolders.view", Params::new()).await?;
        Ok(response
            .music_folders
            .and_then(|container| container.music_folder)
            .unwrap_or_default()
            .into_iter()
            .map(normalize::music_folder)
            .collect())
    }

    pub(super) async fn search_everything(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<SearchResults> {
        let first_results = SearchWindow {
            offset: 0,
            count: limit,
        };
        let request = SearchRequest {
            query,
            artists: first_results,
            albums: first_results,
            songs: first_results,
            music_folder_id: None,
        };
        let result = self.search3(&request).await?;
        Ok(SearchResults {
            songs: self.normalize_songs(result.song.unwrap_or_default()),
            albums: self.normalize_albums(result.album.unwrap_or_default()),
            artists: self.normalize_artists(result.artist.unwrap_or_default()),
        })
    }

    pub(super) async fn fetch_lyrics(&self, song: &Song) -> Result<Option<String>> {
        let extensions = self.extensions().await?;
        if !extensions.supports_song_lyrics() {
            return Ok(None);
        }
        let params = Params::new().add("id", &song.id).opt(
            "enhanced",
            extensions.supports_enhanced_lyrics().then_some(true),
        );
        let response: LyricsList = match self.fetch("getLyricsBySongId.view", params).await {
            Ok(response) => response,
            Err(ApiError::NotFound) => return Ok(None),
            Err(error) => return Err(error),
        };
        let lyrics = response
            .lyrics_list
            .and_then(|container| container.structured_lyrics)
            .unwrap_or_default();
        Ok(lyrics_to_lrc_or_plain_text(&lyrics))
    }
}
