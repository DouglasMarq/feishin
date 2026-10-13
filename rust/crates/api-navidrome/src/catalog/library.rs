use feishin_api::Result;
use feishin_core::model::{Genre, MusicFolder, SearchResults};

use crate::NavidromeServer;
use crate::native::{NativeTag, normalize_genre};
use crate::query::QueryParams;
use crate::subsonic;

impl NavidromeServer {
    pub(crate) async fn list_genres(&self) -> Result<Vec<Genre>> {
        let features = self.features().await;
        if !features.has_big_feature_release() {
            return self.subsonic_genres().await;
        }
        self.native_or_subsonic(self.native_genres(), self.subsonic_genres())
            .await
    }

    pub(crate) async fn list_music_folders(&self) -> Result<Vec<MusicFolder>> {
        let response: subsonic::MusicFolders = self.subsonic("getMusicFolders", &[]).await?;
        Ok(response
            .music_folders
            .unwrap_or_default()
            .music_folder
            .into_iter()
            .map(subsonic::normalize_music_folder)
            .collect())
    }

    pub(crate) async fn search_library(&self, query: &str, limit: usize) -> Result<SearchResults> {
        let params: QueryParams = vec![
            ("query", query.to_owned()),
            ("songCount", limit.to_string()),
            ("albumCount", limit.to_string()),
            ("artistCount", limit.to_string()),
        ];
        let response: subsonic::Search3 = self.subsonic("search3", &params).await?;
        let result = response.search_result3.unwrap_or_default();
        Ok(SearchResults {
            songs: self.songs_from_subsonic(result.song),
            albums: result
                .album
                .into_iter()
                .map(|album| subsonic::normalize_album(album, self.server_id()))
                .collect(),
            artists: result
                .artist
                .into_iter()
                .map(|artist| subsonic::normalize_artist(artist, self.server_id()))
                .collect(),
        })
    }

    async fn subsonic_genres(&self) -> Result<Vec<Genre>> {
        let response: subsonic::Genres = self.subsonic("getGenres", &[]).await?;
        let mut genres: Vec<Genre> = response
            .genres
            .unwrap_or_default()
            .genre
            .into_iter()
            .map(subsonic::normalize_genre)
            .collect();
        genres.sort_by_key(|genre| genre.name.to_lowercase());
        Ok(genres)
    }

    async fn native_genres(&self) -> Result<Vec<Genre>> {
        let params: QueryParams = vec![
            ("_sort", "tagValue".into()),
            ("_order", "ASC".into()),
            ("tag_name", "genre".into()),
        ];
        let (tags, _): (Vec<NativeTag>, _) = self.native_get("tag", &params).await?;
        Ok(tags.into_iter().map(normalize_genre).collect())
    }
}
