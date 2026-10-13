//! Navidrome backend for `feishin_api::MusicServer`.
//!
//! Lists, counts and details use the native API (`/api/*`) authenticated with the session JWT,
//! which Navidrome refreshes through the `x-nd-authorization` response header. Everything else, and
//! every call made without a valid JWT, goes through the Subsonic API (`/rest/*.view`).

mod client;
mod features;
mod native;
mod query;
mod subsonic;
mod util;

use std::sync::RwLock;

use async_trait::async_trait;
use feishin_api::{ApiError, Credential, MusicServer, Result, StreamOptions};
use feishin_core::model::{
    Album, AlbumDetail, AlbumListQuery, AlbumSort, Artist, ArtistDetail, ArtistListQuery, Genre,
    ItemKind, MusicFolder, Page, Paging, Playlist, SearchResults, ServerConfig, ServerPlayQueue,
    Song, SongListQuery, SortOrder,
};
use reqwest::{Method, StatusCode};
use serde::de::IgnoredAny;
use serde_json::json;
use tokio::sync::OnceCell;

use crate::client::{decode, network, no_session, query_string};
use crate::features::{EXT_INDEX_QUEUE, EXT_SONG_LYRICS, EXT_TOP_SONGS_BY_ID, Features};
use crate::native::{NdAlbum, NdArtist, NdAuth, NdId, NdPlaylist, NdQueue, NdSong, NdTag};
use crate::query::{Params, end, end_or_all};

const ALBUM_IDS_BATCH_SIZE: usize = 500;
const SUBSONIC_MAX_ITEMS: usize = 500;

pub struct NavidromeServer {
    config: ServerConfig,
    base_url: String,
    http: reqwest::Client,
    subsonic_auth: String,
    nd_token: RwLock<Option<String>>,
    features: OnceCell<Features>,
}

impl NavidromeServer {
    pub async fn login(config: ServerConfig, password: &str) -> Result<(Self, Credential)> {
        let http = build_http()?;
        let base_url = config.url.trim_end_matches('/').to_owned();
        let response = http
            .post(format!("{base_url}/auth/login"))
            .json(&json!({ "username": config.username, "password": password }))
            .send()
            .await
            .map_err(network)?;

        let status = response.status();
        let body = response.bytes().await.map_err(network)?;
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            return Err(ApiError::Auth("invalid username or password".into()));
        }
        if !status.is_success() {
            return Err(ApiError::Server {
                code: i32::from(status.as_u16()),
                message: String::from_utf8_lossy(&body).trim().to_owned(),
            });
        }

        let auth: NdAuth = serde_json::from_slice(&body).map_err(decode)?;
        let subsonic = query_string(&[
            ("u", config.username.clone()),
            ("s", auth.subsonic_salt),
            ("t", auth.subsonic_token),
        ]);
        let credential = Credential::Navidrome {
            subsonic,
            nd_token: Some(auth.token),
        };
        let server = Self::from_parts(config, http, &credential)?;
        Ok((server, credential))
    }

    pub fn with_credential(config: ServerConfig, credential: &Credential) -> Result<Self> {
        Self::from_parts(config, build_http()?, credential)
    }

    /// The current credential: the JWT is the latest one refreshed by the server, or `None` once it was
    /// rejected. Persist it after use.
    pub fn credential(&self) -> Credential {
        Credential::Navidrome {
            subsonic: self.subsonic_auth.clone(),
            nd_token: self.token(),
        }
    }

    fn from_parts(
        config: ServerConfig,
        http: reqwest::Client,
        credential: &Credential,
    ) -> Result<Self> {
        let Credential::Navidrome { subsonic, nd_token } = credential else {
            return Err(ApiError::Auth("not a Navidrome credential".into()));
        };
        let base_url = config.url.trim_end_matches('/').to_owned();
        Ok(Self {
            config,
            base_url,
            http,
            subsonic_auth: subsonic.trim_start_matches(['?', '&']).to_owned(),
            nd_token: RwLock::new(nd_token.clone().filter(|t| !t.is_empty())),
            features: OnceCell::new(),
        })
    }

    fn server_id(&self) -> &str {
        &self.config.id
    }

    fn songs_from_native(&self, songs: Vec<NdSong>) -> Vec<Song> {
        songs
            .into_iter()
            .map(|s| native::song(s, self.server_id()))
            .collect()
    }

    fn songs_from_subsonic(&self, songs: Vec<subsonic::SsSong>) -> Vec<Song> {
        songs
            .into_iter()
            .map(|s| subsonic::song(s, self.server_id()))
            .collect()
    }

    async fn native_song_page(
        &self,
        query: &SongListQuery,
        paging: Paging,
        features: &Features,
    ) -> Result<Page<Song>> {
        let start = paging.start_index;
        let end = end_or_all(start, paging.limit);
        let batches: Vec<&[String]> = if query.album_ids.len() > ALBUM_IDS_BATCH_SIZE {
            query.album_ids.chunks(ALBUM_IDS_BATCH_SIZE).collect()
        } else {
            vec![query.album_ids.as_slice()]
        };

        let mut items = Vec::new();
        let mut total: Option<usize> = None;
        for batch in batches {
            let params = query::song_list(query, start, end, batch, features);
            let (songs, count): (Vec<NdSong>, _) = self.native_get("song", &params).await?;
            items.extend(self.songs_from_native(songs));
            if let Some(count) = count {
                total = Some(total.unwrap_or(0) + count);
            }
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
        let filtered = query.favorite.is_some()
            || !query.genre_ids.is_empty()
            || !query.artist_ids.is_empty()
            || !query.album_ids.is_empty()
            || query.year_from.is_some()
            || query.year_to.is_some();
        if filtered {
            return Err(no_session());
        }

        let limit = if paging.limit == 0 {
            SUBSONIC_MAX_ITEMS
        } else {
            paging.limit
        };
        let mut params: Params = vec![
            ("query", query.search.clone().unwrap_or_default()),
            ("songCount", limit.to_string()),
            ("songOffset", paging.start_index.to_string()),
            ("albumCount", "0".into()),
            ("artistCount", "0".into()),
        ];
        if let Some(folder) = &query.music_folder_id {
            params.push(("musicFolderId", folder.clone()));
        }
        let result: subsonic::Search3 = self.subsonic("search3", &params).await?;
        Ok(Page {
            items: self.songs_from_subsonic(result.search_result3.unwrap_or_default().song),
            start_index: paging.start_index,
            total_count: None,
        })
    }

    async fn native_album_page(
        &self,
        query: &AlbumListQuery,
        start: usize,
        end: i64,
        features: &Features,
    ) -> Result<Page<Album>> {
        let params = query::album_list(query, start, end, features);
        let (albums, total): (Vec<NdAlbum>, _) = self.native_get("album", &params).await?;
        Ok(Page {
            items: albums
                .into_iter()
                .map(|a| native::album(a, self.server_id()))
                .collect(),
            start_index: start,
            total_count: total,
        })
    }

    async fn subsonic_album_page(
        &self,
        query: &AlbumListQuery,
        paging: Paging,
    ) -> Result<Page<Album>> {
        let filtered = !query.artist_ids.is_empty()
            || !query.genre_ids.is_empty()
            || query.search.as_ref().is_some_and(|s| !s.is_empty())
            || query.year_from.is_some()
            || query.year_to.is_some()
            || query.favorite == Some(false);
        let kind = if query.favorite == Some(true) {
            Some("starred")
        } else {
            query::subsonic_album_type(query.sort)
        };
        let Some(kind) = kind.filter(|_| !filtered) else {
            return Err(no_session());
        };

        let limit = if paging.limit == 0 {
            SUBSONIC_MAX_ITEMS
        } else {
            paging.limit.min(SUBSONIC_MAX_ITEMS)
        };
        let mut params: Params = vec![
            ("type", kind.to_owned()),
            ("size", limit.to_string()),
            ("offset", paging.start_index.to_string()),
        ];
        if kind == "byYear" {
            let (from, to) = match query.order {
                SortOrder::Asc => ("0", "9999"),
                SortOrder::Desc => ("9999", "0"),
            };
            params.push(("fromYear", from.into()));
            params.push(("toYear", to.into()));
        }
        if let Some(folder) = &query.music_folder_id {
            params.push(("musicFolderId", folder.clone()));
        }
        let result: subsonic::AlbumList2 = self.subsonic("getAlbumList2", &params).await?;
        Ok(Page {
            items: result
                .album_list2
                .unwrap_or_default()
                .album
                .into_iter()
                .map(|a| subsonic::album(a, self.server_id()))
                .collect(),
            start_index: paging.start_index,
            total_count: None,
        })
    }

    async fn native_album(&self, id: &str, features: &Features) -> Result<AlbumDetail> {
        let (album, _): (NdAlbum, _) = self.native_get(&format!("album/{id}"), &[]).await?;
        let (songs, _): (Vec<NdSong>, _) = self
            .native_get("song", &query::album_songs(id, features))
            .await?;
        Ok(AlbumDetail {
            album: native::album(album, self.server_id()),
            songs: self.songs_from_native(songs),
        })
    }

    async fn subsonic_album(&self, id: &str) -> Result<AlbumDetail> {
        let response: subsonic::AlbumResponse =
            self.subsonic("getAlbum", &[("id", id.to_owned())]).await?;
        let mut album = response.album.ok_or(ApiError::NotFound)?;
        let songs = album.song.take().unwrap_or_default();
        Ok(AlbumDetail {
            album: subsonic::album(album, self.server_id()),
            songs: self.songs_from_subsonic(songs),
        })
    }

    async fn top_songs(
        &self,
        artist_id: &str,
        name: &str,
        features: &Features,
    ) -> Result<Vec<Song>> {
        let params: Params = if features.has_extension(EXT_TOP_SONGS_BY_ID) {
            vec![("id", artist_id.to_owned())]
        } else {
            vec![("artist", name.to_owned())]
        };
        match self
            .subsonic::<subsonic::TopSongs>("getTopSongs", &params)
            .await
        {
            Ok(top) => Ok(self.songs_from_subsonic(top.top_songs.unwrap_or_default().song)),
            Err(ApiError::Server { .. } | ApiError::NotFound) => Ok(Vec::new()),
            Err(e) => Err(e),
        }
    }

    async fn native_artist(&self, id: &str, features: &Features) -> Result<ArtistDetail> {
        let (artist, _): (NdArtist, _) = self.native_get(&format!("artist/{id}"), &[]).await?;
        let albums_query = AlbumListQuery {
            sort: AlbumSort::Year,
            order: SortOrder::Desc,
            artist_ids: vec![id.to_owned()],
            ..AlbumListQuery::default()
        };
        let albums = self
            .native_album_page(&albums_query, 0, -1, features)
            .await?
            .items;
        let artist = native::artist(artist, self.server_id());
        let top_songs = self.top_songs(id, &artist.name, features).await?;
        Ok(ArtistDetail {
            artist,
            albums,
            top_songs,
        })
    }

    async fn subsonic_artist(&self, id: &str, features: &Features) -> Result<ArtistDetail> {
        let response: subsonic::ArtistResponse =
            self.subsonic("getArtist", &[("id", id.to_owned())]).await?;
        let mut artist = response.artist.ok_or(ApiError::NotFound)?;
        let albums = artist
            .album
            .take()
            .unwrap_or_default()
            .into_iter()
            .map(|a| subsonic::album(a, self.server_id()))
            .collect();
        let mut artist = subsonic::artist(artist, self.server_id());
        if let Ok(info) = self
            .subsonic::<subsonic::ArtistInfo2>("getArtistInfo2", &[("id", id.to_owned())])
            .await
        {
            artist.biography = info
                .artist_info2
                .and_then(|i| i.biography)
                .filter(|b| !b.is_empty());
        }
        let top_songs = self.top_songs(id, &artist.name, features).await?;
        Ok(ArtistDetail {
            artist,
            albums,
            top_songs,
        })
    }

    async fn subsonic_artist_page(
        &self,
        query: &ArtistListQuery,
        paging: Paging,
    ) -> Result<Page<Artist>> {
        let filtered = query.favorite.is_some()
            || query.search.as_ref().is_some_and(|s| !s.is_empty())
            || !matches!(query.sort, feishin_core::model::ArtistSort::Name);
        if filtered {
            return Err(no_session());
        }
        let mut params = Params::new();
        if let Some(folder) = &query.music_folder_id {
            params.push(("musicFolderId", folder.clone()));
        }
        let response: subsonic::Artists = self.subsonic("getArtists", &params).await?;
        let mut artists: Vec<Artist> = response
            .artists
            .unwrap_or_default()
            .index
            .into_iter()
            .flat_map(|i| i.artist)
            .map(|a| subsonic::artist(a, self.server_id()))
            .collect();
        if query.order == SortOrder::Desc {
            artists.reverse();
        }
        Ok(paginate(artists, paging))
    }

    async fn subsonic_genres(&self) -> Result<Vec<Genre>> {
        let response: subsonic::Genres = self.subsonic("getGenres", &[]).await?;
        let mut genres: Vec<Genre> = response
            .genres
            .unwrap_or_default()
            .genre
            .into_iter()
            .map(subsonic::genre)
            .collect();
        genres.sort_by_key(|g| g.name.to_lowercase());
        Ok(genres)
    }

    async fn native_genres(&self) -> Result<Vec<Genre>> {
        let params: Params = vec![
            ("_sort", "tagValue".into()),
            ("_order", "ASC".into()),
            ("tag_name", "genre".into()),
        ];
        let (tags, _): (Vec<NdTag>, _) = self.native_get("tag", &params).await?;
        Ok(tags.into_iter().map(native::genre).collect())
    }

    async fn native_playlist(&self, id: &str) -> Result<Playlist> {
        let (playlist, _): (NdPlaylist, _) =
            self.native_get(&format!("playlist/{id}"), &[]).await?;
        Ok(native::playlist(playlist, self.server_id()))
    }

    async fn native_playlist_tracks(&self, id: &str, features: &Features) -> Result<Vec<NdSong>> {
        let (tracks, _): (Vec<NdSong>, _) = self
            .native_get(
                &format!("playlist/{id}/tracks"),
                &query::playlist_tracks(features),
            )
            .await?;
        Ok(tracks)
    }

    async fn native_add_tracks(&self, playlist_id: &str, song_ids: &[String]) -> Result<()> {
        self.native(
            Method::POST,
            &format!("playlist/{playlist_id}/tracks"),
            &[],
            Some(&json!({ "ids": song_ids })),
        )
        .await
        .map(|_| ())
    }

    async fn native_create_playlist(&self, name: &str, song_ids: &[String]) -> Result<Playlist> {
        let created: NdId = self
            .native(
                Method::POST,
                "playlist",
                &[],
                Some(&json!({ "name": name, "rules": null })),
            )
            .await?
            .json()?;
        if !song_ids.is_empty() {
            self.native_add_tracks(&created.id, song_ids).await?;
        }
        self.native_playlist(&created.id).await
    }

    async fn subsonic_create_playlist(&self, name: &str, song_ids: &[String]) -> Result<Playlist> {
        let features = self.features().await;
        let mut params: Params = vec![("name", name.to_owned())];
        params.extend(song_ids.iter().map(|id| ("songId", id.clone())));
        let response: subsonic::PlaylistResponse = self
            .subsonic_large("createPlaylist", &params, &features)
            .await?;
        let playlist = response
            .playlist
            .ok_or_else(|| ApiError::Decode("createPlaylist returned no playlist".into()))?;
        Ok(subsonic::playlist(playlist, self.server_id()))
    }

    async fn native_remove_tracks(&self, playlist_id: &str, indexes: &[usize]) -> Result<()> {
        let features = self.features().await;
        let tracks = self.native_playlist_tracks(playlist_id, &features).await?;
        let mut params = Params::new();
        for index in indexes {
            let track = tracks.get(*index).ok_or(ApiError::NotFound)?;
            params.push(("id", track.id.clone()));
        }
        self.native(
            Method::DELETE,
            &format!("playlist/{playlist_id}/tracks"),
            &params,
            None,
        )
        .await
        .map(|_| ())
    }

    async fn subsonic_play_queue(&self, features: &Features) -> Result<Option<ServerPlayQueue>> {
        let (body, by_index) = if features.has_extension(EXT_INDEX_QUEUE) {
            let response: subsonic::PlayQueueByIndex =
                self.subsonic("getPlayQueueByIndex", &[]).await?;
            (response.play_queue_by_index, true)
        } else {
            match self
                .subsonic::<subsonic::PlayQueue>("getPlayQueue", &[])
                .await
            {
                Ok(response) => (response.play_queue, false),
                Err(ApiError::NotFound) => (None, false),
                Err(e) => return Err(e),
            }
        };
        let Some(body) = body else {
            return Ok(None);
        };
        let songs = self.songs_from_subsonic(body.entry.unwrap_or_default());
        if songs.is_empty() {
            return Ok(None);
        }
        let current_song_id = if by_index {
            body.current_index
                .and_then(|i| songs.get(i as usize))
                .map(|s| s.id.clone())
        } else {
            body.current
        };
        Ok(Some(ServerPlayQueue {
            songs,
            current_song_id,
            position_ms: body.position.unwrap_or(0),
        }))
    }

    async fn native_play_queue(&self) -> Result<Option<ServerPlayQueue>> {
        let (queue, _): (NdQueue, _) = self.native_get("queue", &[]).await?;
        let songs = self.songs_from_native(queue.items.unwrap_or_default());
        if songs.is_empty() {
            return Ok(None);
        }
        let current = queue.current.unwrap_or(0) as usize;
        Ok(Some(ServerPlayQueue {
            current_song_id: songs.get(current).map(|s| s.id.clone()),
            songs,
            position_ms: queue.position.unwrap_or(0),
        }))
    }

    async fn subsonic_save_queue(
        &self,
        song_ids: &[String],
        current: Option<usize>,
        position_ms: u64,
        features: &Features,
    ) -> Result<()> {
        let mut params: Params = song_ids.iter().map(|id| ("id", id.clone())).collect();
        params.push(("position", position_ms.to_string()));
        let endpoint = if features.has_extension(EXT_INDEX_QUEUE) {
            if let Some(index) = current {
                params.push(("currentIndex", index.to_string()));
            }
            "savePlayQueueByIndex"
        } else {
            if let Some(id) = current.and_then(|i| song_ids.get(i)) {
                params.push(("current", id.clone()));
            }
            "savePlayQueue"
        };
        self.subsonic_large::<IgnoredAny>(endpoint, &params, features)
            .await
            .map(|_| ())
    }
}

fn build_http() -> Result<reqwest::Client> {
    reqwest::Client::builder().build().map_err(network)
}

fn paginate<T>(items: Vec<T>, paging: Paging) -> Page<T> {
    let total = items.len();
    let take = if paging.limit == 0 {
        total
    } else {
        paging.limit
    };
    Page {
        items: items
            .into_iter()
            .skip(paging.start_index)
            .take(take)
            .collect(),
        start_index: paging.start_index,
        total_count: Some(total),
    }
}

fn favorite_key(kind: ItemKind) -> Result<&'static str> {
    match kind {
        ItemKind::Song => Ok("id"),
        ItemKind::Album => Ok("albumId"),
        ItemKind::Artist => Ok("artistId"),
        ItemKind::Playlist => Err(ApiError::Unsupported("favorite playlists")),
    }
}

#[async_trait]
impl MusicServer for NavidromeServer {
    fn config(&self) -> &ServerConfig {
        &self.config
    }

    async fn ping(&self) -> Result<()> {
        self.subsonic::<IgnoredAny>("ping", &[]).await.map(|_| ())
    }

    async fn song_list(&self, query: &SongListQuery, paging: Paging) -> Result<Page<Song>> {
        let features = self.features().await;
        self.native_or(
            self.native_song_page(query, paging, &features),
            self.subsonic_song_page(query, paging),
        )
        .await
    }

    async fn song_count(&self, query: &SongListQuery) -> Result<usize> {
        let features = self.features().await;
        let paging = Paging {
            start_index: 0,
            limit: 1,
        };
        let page = self.native_song_page(query, paging, &features).await?;
        Ok(page.total_count.unwrap_or(0))
    }

    async fn song(&self, id: &str) -> Result<Song> {
        let native = async {
            let (song, _): (NdSong, _) = self.native_get(&format!("song/{id}"), &[]).await?;
            Ok(native::song(song, self.server_id()))
        };
        let fallback = async {
            let response: subsonic::SongResponse =
                self.subsonic("getSong", &[("id", id.to_owned())]).await?;
            let song = response.song.ok_or(ApiError::NotFound)?;
            Ok(subsonic::song(song, self.server_id()))
        };
        self.native_or(native, fallback).await
    }

    async fn random_songs(&self, count: usize, genre: Option<&str>) -> Result<Vec<Song>> {
        let mut params: Params = vec![("size", count.to_string())];
        if let Some(genre) = genre {
            params.push(("genre", genre.to_owned()));
        }
        let response: subsonic::RandomSongs = self.subsonic("getRandomSongs", &params).await?;
        Ok(self.songs_from_subsonic(response.random_songs.unwrap_or_default().song))
    }

    async fn similar_songs(&self, song_id: &str, count: usize) -> Result<Vec<Song>> {
        let params: Params = vec![("id", song_id.to_owned()), ("count", count.to_string())];
        let response: subsonic::SimilarSongs = self.subsonic("getSimilarSongs", &params).await?;
        let songs = response
            .similar_songs
            .unwrap_or_default()
            .song
            .into_iter()
            .filter(|s| s.id != song_id)
            .collect();
        Ok(self.songs_from_subsonic(songs))
    }

    async fn album_list(&self, query: &AlbumListQuery, paging: Paging) -> Result<Page<Album>> {
        let features = self.features().await;
        let start = paging.start_index;
        self.native_or(
            self.native_album_page(query, start, end(start, paging.limit), &features),
            self.subsonic_album_page(query, paging),
        )
        .await
    }

    async fn album_count(&self, query: &AlbumListQuery) -> Result<usize> {
        let features = self.features().await;
        let page = self.native_album_page(query, 0, 1, &features).await?;
        Ok(page.total_count.unwrap_or(0))
    }

    async fn album(&self, id: &str) -> Result<AlbumDetail> {
        let features = self.features().await;
        self.native_or(self.native_album(id, &features), self.subsonic_album(id))
            .await
    }

    async fn artist_list(&self, query: &ArtistListQuery, paging: Paging) -> Result<Page<Artist>> {
        let features = self.features().await;
        let start = paging.start_index;
        let native = async {
            let params = query::artist_list(query, start, end(start, paging.limit), &features);
            let (artists, total): (Vec<NdArtist>, _) = self.native_get("artist", &params).await?;
            Ok(Page {
                items: artists
                    .into_iter()
                    .map(|a| native::artist(a, self.server_id()))
                    .collect(),
                start_index: start,
                total_count: total,
            })
        };
        self.native_or(native, self.subsonic_artist_page(query, paging))
            .await
    }

    async fn artist(&self, id: &str) -> Result<ArtistDetail> {
        let features = self.features().await;
        self.native_or(
            self.native_artist(id, &features),
            self.subsonic_artist(id, &features),
        )
        .await
    }

    async fn genre_list(&self) -> Result<Vec<Genre>> {
        let features = self.features().await;
        if features.bfr() {
            self.native_or(self.native_genres(), self.subsonic_genres())
                .await
        } else {
            self.subsonic_genres().await
        }
    }

    async fn music_folders(&self) -> Result<Vec<MusicFolder>> {
        let response: subsonic::MusicFolders = self.subsonic("getMusicFolders", &[]).await?;
        Ok(response
            .music_folders
            .unwrap_or_default()
            .music_folder
            .into_iter()
            .map(subsonic::music_folder)
            .collect())
    }

    async fn playlist_list(&self, paging: Paging) -> Result<Page<Playlist>> {
        let start = paging.start_index;
        let native = async {
            let params: Params = vec![
                ("_start", start.to_string()),
                ("_end", end(start, paging.limit).to_string()),
                ("_sort", "name".into()),
                ("_order", "ASC".into()),
            ];
            let (playlists, total): (Vec<NdPlaylist>, _) =
                self.native_get("playlist", &params).await?;
            Ok(Page {
                items: playlists
                    .into_iter()
                    .map(|p| native::playlist(p, self.server_id()))
                    .collect(),
                start_index: start,
                total_count: total,
            })
        };
        let fallback = async {
            let response: subsonic::Playlists = self.subsonic("getPlaylists", &[]).await?;
            let playlists = response
                .playlists
                .unwrap_or_default()
                .playlist
                .into_iter()
                .map(|p| subsonic::playlist(p, self.server_id()))
                .collect();
            Ok(paginate(playlists, paging))
        };
        self.native_or(native, fallback).await
    }

    async fn playlist_songs(&self, id: &str) -> Result<Vec<Song>> {
        let features = self.features().await;
        let native = async {
            let tracks = self.native_playlist_tracks(id, &features).await?;
            Ok(self.songs_from_native(tracks))
        };
        let fallback = async {
            let response: subsonic::PlaylistResponse = self
                .subsonic("getPlaylist", &[("id", id.to_owned())])
                .await?;
            let playlist = response.playlist.ok_or(ApiError::NotFound)?;
            Ok(self.songs_from_subsonic(playlist.entry.unwrap_or_default()))
        };
        self.native_or(native, fallback).await
    }

    async fn create_playlist(&self, name: &str, song_ids: &[String]) -> Result<Playlist> {
        self.native_or(
            self.native_create_playlist(name, song_ids),
            self.subsonic_create_playlist(name, song_ids),
        )
        .await
    }

    async fn add_to_playlist(&self, playlist_id: &str, song_ids: &[String]) -> Result<()> {
        if song_ids.is_empty() {
            return Ok(());
        }
        let fallback = async {
            let features = self.features().await;
            let mut params: Params = vec![("playlistId", playlist_id.to_owned())];
            params.extend(song_ids.iter().map(|id| ("songIdToAdd", id.clone())));
            self.subsonic_large::<IgnoredAny>("updatePlaylist", &params, &features)
                .await
                .map(|_| ())
        };
        self.native_or(self.native_add_tracks(playlist_id, song_ids), fallback)
            .await
    }

    async fn remove_from_playlist(&self, playlist_id: &str, indexes: &[usize]) -> Result<()> {
        if indexes.is_empty() {
            return Ok(());
        }
        let fallback = async {
            let mut params: Params = vec![("playlistId", playlist_id.to_owned())];
            params.extend(indexes.iter().map(|i| ("songIndexToRemove", i.to_string())));
            self.subsonic::<IgnoredAny>("updatePlaylist", &params)
                .await
                .map(|_| ())
        };
        self.native_or(self.native_remove_tracks(playlist_id, indexes), fallback)
            .await
    }

    async fn delete_playlist(&self, id: &str) -> Result<()> {
        let native = async {
            self.native(Method::DELETE, &format!("playlist/{id}"), &[], None)
                .await
                .map(|_| ())
        };
        let fallback = async {
            self.subsonic::<IgnoredAny>("deletePlaylist", &[("id", id.to_owned())])
                .await
                .map(|_| ())
        };
        self.native_or(native, fallback).await
    }

    async fn search(&self, query: &str, limit: usize) -> Result<SearchResults> {
        let params: Params = vec![
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
                .map(|a| subsonic::album(a, self.server_id()))
                .collect(),
            artists: result
                .artist
                .into_iter()
                .map(|a| subsonic::artist(a, self.server_id()))
                .collect(),
        })
    }

    async fn set_favorite(&self, kind: ItemKind, ids: &[String], favorite: bool) -> Result<()> {
        let key = favorite_key(kind)?;
        if ids.is_empty() {
            return Ok(());
        }
        let params: Params = ids.iter().map(|id| (key, id.clone())).collect();
        let endpoint = if favorite { "star" } else { "unstar" };
        self.subsonic::<IgnoredAny>(endpoint, &params)
            .await
            .map(|_| ())
    }

    async fn set_rating(&self, kind: ItemKind, id: &str, rating: u8) -> Result<()> {
        if kind == ItemKind::Playlist {
            return Err(ApiError::Unsupported("rating playlists"));
        }
        let params: Params = vec![("id", id.to_owned()), ("rating", rating.min(5).to_string())];
        self.subsonic::<IgnoredAny>("setRating", &params)
            .await
            .map(|_| ())
    }

    async fn scrobble(
        &self,
        song_id: &str,
        submission: bool,
        timestamp_ms: Option<i64>,
    ) -> Result<()> {
        let mut params: Params = vec![
            ("id", song_id.to_owned()),
            ("submission", submission.to_string()),
        ];
        if let Some(time) = timestamp_ms {
            params.push(("time", time.to_string()));
        }
        self.subsonic::<IgnoredAny>("scrobble", &params)
            .await
            .map(|_| ())
    }

    async fn save_play_queue(
        &self,
        song_ids: &[String],
        current_song_id: Option<&str>,
        position_ms: u64,
    ) -> Result<()> {
        let features = self.features().await;
        let current = current_song_id.and_then(|c| song_ids.iter().position(|id| id == c));

        if features.native_play_queue() && !features.has_extension(EXT_INDEX_QUEUE) {
            let native = async {
                let mut body = json!({ "ids": song_ids, "position": position_ms });
                if let Some(index) = current {
                    body["current"] = json!(index);
                }
                self.native(Method::POST, "queue", &[], Some(&body))
                    .await
                    .map(|_| ())
            };
            return self
                .native_or(
                    native,
                    self.subsonic_save_queue(song_ids, current, position_ms, &features),
                )
                .await;
        }
        self.subsonic_save_queue(song_ids, current, position_ms, &features)
            .await
    }

    async fn play_queue(&self) -> Result<Option<ServerPlayQueue>> {
        let features = self.features().await;
        if features.native_play_queue() {
            self.native_or(
                self.native_play_queue(),
                self.subsonic_play_queue(&features),
            )
            .await
        } else {
            self.subsonic_play_queue(&features).await
        }
    }

    async fn lyrics(&self, song: &Song) -> Result<Option<String>> {
        let features = self.features().await;
        if !features.has_extension(EXT_SONG_LYRICS) {
            return Ok(None);
        }
        let response: subsonic::LyricsList = self
            .subsonic("getLyricsBySongId", &[("id", song.id.clone())])
            .await?;
        Ok(subsonic::lyrics_text(
            response.lyrics_list.unwrap_or_default().structured_lyrics,
        ))
    }

    fn stream_url(&self, song: &Song, options: &StreamOptions) -> String {
        let mut params: Params = vec![("id", song.id.clone())];
        if let Some(format) = &options.format {
            params.push(("format", format.clone()));
        }
        if let Some(bitrate) = options.max_bitrate_kbps {
            params.push(("maxBitRate", bitrate.to_string()));
        }
        self.subsonic_url("stream", &params, false)
    }

    fn cover_art_url(&self, cover_art_id: &str, size: Option<u32>) -> String {
        let mut params: Params = match cover_art_id.split_once("&_=") {
            Some((id, bust)) => vec![("id", id.to_owned()), ("_", bust.to_owned())],
            None => vec![("id", cover_art_id.to_owned())],
        };
        if let Some(size) = size {
            params.push(("size", size.to_string()));
        }
        self.subsonic_url("getCoverArt", &params, false)
    }
}
