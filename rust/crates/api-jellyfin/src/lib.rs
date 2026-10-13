//! Jellyfin backend for `feishin_api::MusicServer`.

mod dto;
mod normalize;

use std::collections::HashSet;

use async_trait::async_trait;
use feishin_api::{ApiError, Credential, MusicServer, Result, StreamOptions};
use feishin_core::model::{
    AlbumDetail, AlbumListQuery, AlbumSort, Artist, ArtistDetail, ArtistListQuery, ArtistSort,
    Genre, ItemKind, MusicFolder, Page, Paging, Playlist, SearchResults, ServerConfig,
    ServerPlayQueue, Song, SongListQuery, SongSort, SortOrder,
};
use reqwest::{Method, RequestBuilder, Response, StatusCode, header::AUTHORIZATION};
use serde::de::DeserializeOwned;
use serde_json::json;

use crate::normalize::TICKS_PER_MS;

const CLIENT_NAME: &str = "Feishin";
const DEVICE_NAME: &str = "Desktop Client";
const CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Ids are sent in the query string, so batches stay small to keep URLs short.
const MAX_IDS_PER_REQUEST: usize = 50;

const SONG_FIELDS: &[&str] = &[
    "Genres",
    "DateCreated",
    "MediaSources",
    "ParentId",
    "Tags",
    "SortName",
    "ProviderIds",
];
const ALBUM_LIST_FIELDS: &[&str] = &["Tags", "Studios", "SortName", "ProviderIds", "ChildCount"];
const ALBUM_DETAIL_FIELDS: &[&str] =
    &["Genres", "DateCreated", "ChildCount", "Tags", "ProviderIds"];
const ARTIST_LIST_FIELDS: &[&str] = &[
    "Genres",
    "DateCreated",
    "ExternalUrls",
    "Overview",
    "SortName",
    "ProviderIds",
];
const ARTIST_DETAIL_FIELDS: &[&str] = &["Genres", "Overview", "SortName"];
const PLAYLIST_LIST_FIELDS: &[&str] = &[
    "ChildCount",
    "Genres",
    "DateCreated",
    "ParentId",
    "Overview",
];
const PLAYLIST_DETAIL_FIELDS: &[&str] = &[
    "Genres",
    "DateCreated",
    "MediaSources",
    "ChildCount",
    "ParentId",
    "SortName",
];

const SONG_ITEM_TYPES: &str = "Audio";
const SONG_SORT_FALLBACK: &str = "Album,SortName";
const ALBUM_SORT_FALLBACK: &str = "SortName";
const ARTIST_SORT_FALLBACK: &str = "SortName,Name";

pub struct JellyfinServer {
    config: ServerConfig,
    base_url: String,
    access_token: String,
    user_id: String,
    http: reqwest::Client,
}

/// The `MediaBrowser` Authorization header Feishin sends on every request.
pub fn auth_header(device_id: &str, token: Option<&str>) -> String {
    let mut header = format!(
        r#"MediaBrowser Client="{CLIENT_NAME}", Device="{DEVICE_NAME}", DeviceId="{device_id}", Version="{CLIENT_VERSION}""#
    );
    if let Some(token) = token {
        header.push_str(&format!(r#", Token="{token}""#));
    }
    header
}

fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .build()
        .map_err(|e| ApiError::Network(e.to_string()))
}

fn base_url(config: &ServerConfig) -> String {
    config.url.trim_end_matches('/').to_owned()
}

async fn send(request: RequestBuilder) -> Result<Response> {
    let response = request
        .send()
        .await
        .map_err(|e| ApiError::Network(e.to_string()))?;
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let message = response.text().await.unwrap_or_default();
    Err(match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => ApiError::Auth(if message.is_empty() {
            status.to_string()
        } else {
            message
        }),
        StatusCode::NOT_FOUND => ApiError::NotFound,
        _ => ApiError::Server {
            code: i32::from(status.as_u16()),
            message,
        },
    })
}

async fn decode<T: DeserializeOwned>(response: Response) -> Result<T> {
    let bytes = response
        .bytes()
        .await
        .map_err(|e| ApiError::Network(e.to_string()))?;
    serde_json::from_slice(&bytes).map_err(|e| ApiError::Decode(e.to_string()))
}

#[derive(Debug, Default, Clone)]
struct Params(Vec<(&'static str, String)>);

impl Params {
    fn set(mut self, key: &'static str, value: impl ToString) -> Self {
        self.0.push((key, value.to_string()));
        self
    }

    fn opt(self, key: &'static str, value: Option<impl ToString>) -> Self {
        match value {
            Some(value) => self.set(key, value),
            None => self,
        }
    }

    fn fields(self, fields: &[&str]) -> Self {
        fields
            .iter()
            .fold(self, |params, field| params.set("Fields", field))
    }
}

fn join(ids: &[String]) -> Option<String> {
    (!ids.is_empty()).then(|| ids.join(","))
}

fn years(from: Option<i32>, to: Option<i32>) -> Option<String> {
    let (from, to) = (from.filter(|y| *y != 0)?, to.filter(|y| *y != 0)?);
    let years: Vec<String> = (from..=to).map(|year| year.to_string()).collect();
    (!years.is_empty()).then(|| years.join(","))
}

fn sort_order(order: SortOrder) -> &'static str {
    match order {
        SortOrder::Asc => "Ascending",
        SortOrder::Desc => "Descending",
    }
}

fn song_sort(sort: SongSort) -> &'static str {
    match sort {
        SongSort::Name => "Name",
        SongSort::Album => "Album,SortName",
        SongSort::AlbumArtist => "AlbumArtist,Album,SortName",
        SongSort::Artist => "Artist,Album,SortName",
        SongSort::Duration => "Runtime,AlbumArtist,Album,SortName",
        SongSort::Year => "PremiereDate,AlbumArtist,Album,SortName",
        SongSort::PlayCount => "PlayCount,SortName",
        SongSort::RecentlyAdded => "DateCreated,SortName",
        SongSort::RecentlyPlayed => "DatePlayed,SortName",
        SongSort::Random => "Random,SortName",
        SongSort::Rating | SongSort::Favorited => SONG_SORT_FALLBACK,
    }
}

fn album_sort(sort: AlbumSort) -> &'static str {
    match sort {
        AlbumSort::Name => "SortName",
        AlbumSort::AlbumArtist => "AlbumArtist,SortName",
        AlbumSort::Year => "ProductionYear,PremiereDate,SortName",
        AlbumSort::PlayCount => "PlayCount",
        AlbumSort::RecentlyAdded => "DateCreated,SortName",
        AlbumSort::Random => "Random,SortName",
        AlbumSort::Rating
        | AlbumSort::Favorited
        | AlbumSort::RecentlyPlayed
        | AlbumSort::SongCount
        | AlbumSort::Duration => ALBUM_SORT_FALLBACK,
    }
}

fn artist_sort(sort: ArtistSort) -> &'static str {
    match sort {
        ArtistSort::Random => "Random,SortName",
        ArtistSort::Name
        | ArtistSort::AlbumCount
        | ArtistSort::SongCount
        | ArtistSort::PlayCount
        | ArtistSort::Rating
        | ArtistSort::Favorited => ARTIST_SORT_FALLBACK,
    }
}

impl JellyfinServer {
    pub async fn login(config: ServerConfig, password: &str) -> Result<(Self, Credential)> {
        let http = http_client()?;
        let base_url = base_url(&config);
        let response = send(
            http.post(format!("{base_url}/users/authenticatebyname"))
                .header(AUTHORIZATION, auth_header(&config.id, None))
                .json(&json!({ "Pw": password, "Username": config.username })),
        )
        .await?;
        let auth: dto::AuthResponse = decode(response).await?;
        let credential = Credential::Jellyfin {
            access_token: auth.access_token.clone(),
            user_id: auth.user.id.clone(),
        };
        let server = Self {
            config,
            base_url,
            access_token: auth.access_token,
            user_id: auth.user.id,
            http,
        };
        Ok((server, credential))
    }

    pub fn with_credential(config: ServerConfig, credential: &Credential) -> Result<Self> {
        let Credential::Jellyfin {
            access_token,
            user_id,
        } = credential
        else {
            return Err(ApiError::Auth("not a Jellyfin credential".into()));
        };
        Ok(Self {
            base_url: base_url(&config),
            access_token: access_token.clone(),
            user_id: user_id.clone(),
            http: http_client()?,
            config,
        })
    }

    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    /// Reports playback progress (`sessions/playing/progress`), as Feishin does on pause, unpause and time updates.
    pub async fn report_progress(
        &self,
        song_id: &str,
        position_ms: u64,
        paused: bool,
    ) -> Result<()> {
        let event = if paused { "pause" } else { "unpause" };
        self.report(
            "sessions/playing/progress",
            &dto::PlaybackReport {
                item_id: song_id,
                event_name: Some(event),
                is_paused: Some(paused),
                position_ticks: Some(position_ms * TICKS_PER_MS),
            },
        )
        .await
    }

    /// Reports that playback stopped (`sessions/playing/stopped`) at `position_ms`.
    pub async fn report_stopped(&self, song_id: &str, position_ms: u64) -> Result<()> {
        self.report(
            "sessions/playing/stopped",
            &dto::PlaybackReport {
                item_id: song_id,
                event_name: None,
                is_paused: None,
                position_ticks: Some(position_ms * TICKS_PER_MS),
            },
        )
        .await
    }

    async fn report(&self, path: &str, body: &dto::PlaybackReport<'_>) -> Result<()> {
        send(self.request(Method::POST, path).json(body)).await?;
        Ok(())
    }

    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        self.http
            .request(method, format!("{}/{path}", self.base_url))
            .header(
                AUTHORIZATION,
                auth_header(&self.config.id, Some(&self.access_token)),
            )
    }

    async fn get<T: DeserializeOwned>(&self, path: &str, params: &Params) -> Result<T> {
        decode(send(self.request(Method::GET, path).query(&params.0)).await?).await
    }

    fn items_path(&self) -> String {
        format!("users/{}/items", self.user_id)
    }

    fn item_path(&self, id: &str) -> String {
        format!("users/{}/items/{id}", self.user_id)
    }

    fn server_id(&self) -> &str {
        &self.config.id
    }

    fn songs(&self, items: Vec<dto::Song>) -> Vec<Song> {
        items
            .into_iter()
            .map(|item| normalize::song(item, self.server_id()))
            .collect()
    }

    fn song_params(
        &self,
        query: &SongListQuery,
        album_ids: &[String],
        start: usize,
        limit: Option<usize>,
    ) -> Params {
        Params::default()
            .opt("AlbumIds", join(album_ids))
            .opt("ArtistIds", join(&query.artist_ids))
            .fields(SONG_FIELDS)
            .opt("GenreIds", join(&query.genre_ids))
            .set("IncludeItemTypes", SONG_ITEM_TYPES)
            .opt("IsFavorite", query.favorite)
            .opt("Limit", limit)
            .opt("ParentId", query.music_folder_id.as_deref())
            .set("Recursive", true)
            .opt("SearchTerm", query.search.as_deref())
            .set("SortBy", song_sort(query.sort))
            .set("SortOrder", sort_order(query.order))
            .set("StartIndex", start)
            .opt("Years", years(query.year_from, query.year_to))
    }

    async fn fetch_songs(
        &self,
        query: &SongListQuery,
        start: usize,
        limit: Option<usize>,
    ) -> Result<(Vec<dto::Song>, usize)> {
        let path = self.items_path();
        if query.album_ids.len() > MAX_IDS_PER_REQUEST {
            let mut items = Vec::new();
            for batch in query.album_ids.chunks(MAX_IDS_PER_REQUEST) {
                let params = self.song_params(query, batch, start, limit);
                let list: dto::ItemList<dto::Song> = self.get(&path, &params).await?;
                items.extend(list.items);
            }
            let total = items.len();
            return Ok((items, total));
        }

        let params = self.song_params(query, &query.album_ids, start, limit);
        let list: dto::ItemList<dto::Song> = self.get(&path, &params).await?;
        if query.album_ids.is_empty() {
            let total = list.total_record_count.unwrap_or(list.items.len());
            return Ok((list.items, total));
        }
        let wanted: HashSet<&str> = query.album_ids.iter().map(String::as_str).collect();
        let items: Vec<dto::Song> = list
            .items
            .into_iter()
            .filter(|item| {
                item.album_id
                    .as_deref()
                    .is_some_and(|id| wanted.contains(id))
            })
            .collect();
        let total = items.len();
        Ok((items, total))
    }

    fn album_params(&self, query: &AlbumListQuery, start: usize, limit: Option<usize>) -> Params {
        Params::default()
            .opt("ArtistIds", join(&query.artist_ids))
            .fields(ALBUM_LIST_FIELDS)
            .opt("GenreIds", join(&query.genre_ids))
            .set("IncludeItemTypes", "MusicAlbum")
            .opt("IsFavorite", query.favorite)
            .opt("Limit", limit)
            .opt("ParentId", query.music_folder_id.as_deref())
            .set("Recursive", true)
            .opt("SearchTerm", query.search.as_deref())
            .set("SortBy", album_sort(query.sort))
            .set("SortOrder", sort_order(query.order))
            .set("StartIndex", start)
            .opt("Years", years(query.year_from, query.year_to))
    }

    async fn fetch_albums(
        &self,
        query: &AlbumListQuery,
        start: usize,
        limit: Option<usize>,
    ) -> Result<Page<feishin_core::model::Album>> {
        let params = self.album_params(query, start, limit);
        let list: dto::ItemList<dto::Album> = self.get(&self.items_path(), &params).await?;
        Ok(Page {
            total_count: list.total_record_count,
            items: list
                .items
                .into_iter()
                .map(|item| normalize::album(item, self.server_id()))
                .collect(),
            start_index: start,
        })
    }

    async fn fetch_artists(&self, params: &Params) -> Result<dto::ItemList<dto::AlbumArtist>> {
        self.get("artists/albumArtists", params).await
    }

    fn artists(&self, items: Vec<dto::AlbumArtist>) -> Vec<Artist> {
        items
            .into_iter()
            .map(|item| normalize::artist(item, self.server_id()))
            .collect()
    }

    async fn playlist_detail(&self, id: &str) -> Result<Playlist> {
        let params = Params::default()
            .fields(PLAYLIST_DETAIL_FIELDS)
            .set("Ids", id);
        let item: dto::Playlist = self.get(&self.item_path(id), &params).await?;
        Ok(normalize::playlist(item, self.server_id()))
    }

    async fn playlist_items(&self, id: &str, fields: &[&str]) -> Result<Vec<dto::Song>> {
        let params = Params::default()
            .fields(fields)
            .set("IncludeItemTypes", SONG_ITEM_TYPES)
            .set("UserId", &self.user_id);
        let list: dto::ItemList<dto::Song> =
            self.get(&format!("playlists/{id}/items"), &params).await?;
        Ok(list.items)
    }

    async fn mix(&self, path: &str, song_id: &str, count: usize) -> Result<Vec<Song>> {
        let params = Params::default()
            .fields(SONG_FIELDS)
            .set("Limit", count)
            .set("UserId", &self.user_id);
        let list: dto::ItemList<dto::Song> = self.get(path, &params).await?;
        Ok(self.songs(
            list.items
                .into_iter()
                .filter(|item| item.id != song_id)
                .collect(),
        ))
    }
}

#[async_trait]
impl MusicServer for JellyfinServer {
    fn config(&self) -> &ServerConfig {
        &self.config
    }

    async fn ping(&self) -> Result<()> {
        send(self.request(Method::GET, "system/info")).await?;
        Ok(())
    }

    async fn song_list(&self, query: &SongListQuery, paging: Paging) -> Result<Page<Song>> {
        let (items, total) = self
            .fetch_songs(query, paging.start_index, Some(paging.limit))
            .await?;
        Ok(Page {
            items: self.songs(items),
            start_index: paging.start_index,
            total_count: Some(total),
        })
    }

    async fn song_count(&self, query: &SongListQuery) -> Result<usize> {
        let limit = query.album_ids.is_empty().then_some(1);
        Ok(self.fetch_songs(query, 0, limit).await?.1)
    }

    async fn song(&self, id: &str) -> Result<Song> {
        let item: dto::Song = self.get(&self.item_path(id), &Params::default()).await?;
        Ok(normalize::song(item, self.server_id()))
    }

    async fn random_songs(&self, count: usize, genre: Option<&str>) -> Result<Vec<Song>> {
        let params = Params::default()
            .fields(SONG_FIELDS)
            .opt("GenreIds", genre)
            .set("IncludeItemTypes", SONG_ITEM_TYPES)
            .set("Limit", count)
            .set("Recursive", true)
            .set("SortBy", song_sort(SongSort::Random))
            .set("SortOrder", sort_order(SortOrder::Asc))
            .set("StartIndex", 0);
        let list: dto::ItemList<dto::Song> = self.get(&self.items_path(), &params).await?;
        Ok(self.songs(list.items))
    }

    async fn similar_songs(&self, song_id: &str, count: usize) -> Result<Vec<Song>> {
        if let Ok(songs) = self
            .mix(&format!("items/{song_id}/similar"), song_id, count)
            .await
            && !songs.is_empty()
        {
            return Ok(songs);
        }
        self.mix(&format!("items/{song_id}/InstantMix"), song_id, count)
            .await
    }

    async fn album_list(
        &self,
        query: &AlbumListQuery,
        paging: Paging,
    ) -> Result<Page<feishin_core::model::Album>> {
        self.fetch_albums(query, paging.start_index, Some(paging.limit))
            .await
    }

    async fn album_count(&self, query: &AlbumListQuery) -> Result<usize> {
        let page = self.fetch_albums(query, 0, Some(1)).await?;
        Ok(page.total_count.unwrap_or(page.items.len()))
    }

    async fn album(&self, id: &str) -> Result<AlbumDetail> {
        let detail_params = Params::default().fields(ALBUM_DETAIL_FIELDS);
        let item: dto::Album = self.get(&self.item_path(id), &detail_params).await?;
        let song_params = Params::default()
            .set("EnableUserData", true)
            .fields(SONG_FIELDS)
            .set("IncludeItemTypes", SONG_ITEM_TYPES)
            .set("ParentId", id)
            .set("Recursive", true)
            .set("SortBy", "ParentIndexNumber,IndexNumber,SortName")
            .set("SortOrder", sort_order(SortOrder::Asc))
            .set("StartIndex", 0)
            .set("UserId", &self.user_id);
        let songs: dto::ItemList<dto::Song> = self.get(&self.items_path(), &song_params).await?;
        let mut album = normalize::album(item, self.server_id());
        let songs = self.songs(songs.items);
        if album.song_count == 0 {
            album.song_count = u32::try_from(songs.len()).unwrap_or(u32::MAX);
        }
        Ok(AlbumDetail { album, songs })
    }

    async fn artist_list(&self, query: &ArtistListQuery, paging: Paging) -> Result<Page<Artist>> {
        let params = Params::default()
            .fields(ARTIST_LIST_FIELDS)
            .set("ImageTypeLimit", 1)
            .opt("IsFavorite", query.favorite)
            .set("Limit", paging.limit)
            .opt("ParentId", query.music_folder_id.as_deref())
            .set("Recursive", true)
            .opt("SearchTerm", query.search.as_deref())
            .set("SortBy", artist_sort(query.sort))
            .set("SortOrder", sort_order(query.order))
            .set("StartIndex", paging.start_index)
            .set("UserId", &self.user_id);
        let list = self.fetch_artists(&params).await?;
        Ok(Page {
            total_count: list.total_record_count,
            items: self.artists(list.items),
            start_index: paging.start_index,
        })
    }

    async fn artist(&self, id: &str) -> Result<ArtistDetail> {
        let params = Params::default().fields(ARTIST_DETAIL_FIELDS);
        let item: dto::AlbumArtist = self.get(&self.item_path(id), &params).await?;
        let albums_query = AlbumListQuery {
            sort: AlbumSort::Year,
            order: SortOrder::Desc,
            artist_ids: vec![id.to_owned()],
            ..AlbumListQuery::default()
        };
        let albums = self.fetch_albums(&albums_query, 0, None).await?.items;
        let top_params = Params::default()
            .set("ArtistIds", id)
            .fields(SONG_FIELDS)
            .set("IncludeItemTypes", SONG_ITEM_TYPES)
            .set("Recursive", true)
            .set("SortBy", "CommunityRating,SortName")
            .set("SortOrder", sort_order(SortOrder::Desc))
            .set("UserId", &self.user_id);
        let top: dto::ItemList<dto::Song> = self.get(&self.items_path(), &top_params).await?;
        Ok(ArtistDetail {
            artist: normalize::artist(item, self.server_id()),
            albums,
            top_songs: self.songs(top.items),
        })
    }

    async fn genre_list(&self) -> Result<Vec<Genre>> {
        let params = Params::default()
            .set("EnableTotalRecordCount", true)
            .fields(&["ItemCounts"])
            .set("Recursive", true)
            .set("SortBy", "SortName")
            .set("SortOrder", sort_order(SortOrder::Asc))
            .set("StartIndex", 0)
            .set("UserId", &self.user_id);
        let list: dto::ItemList<dto::Genre> = self.get("musicgenres", &params).await?;
        Ok(list.items.into_iter().map(normalize::genre).collect())
    }

    async fn music_folders(&self) -> Result<Vec<MusicFolder>> {
        let list: dto::ItemList<dto::MusicFolder> =
            self.get(&self.items_path(), &Params::default()).await?;
        Ok(list
            .items
            .into_iter()
            .filter(|folder| folder.collection_type.as_deref() == Some("music"))
            .map(normalize::music_folder)
            .collect())
    }

    async fn playlist_list(&self, paging: Paging) -> Result<Page<Playlist>> {
        let params = Params::default()
            .fields(PLAYLIST_LIST_FIELDS)
            .set("IncludeItemTypes", "Playlist")
            .set("Limit", paging.limit)
            .set("MediaTypes", "Audio, Unknown")
            .set("Recursive", true)
            .set("SortBy", "SortName")
            .set("SortOrder", sort_order(SortOrder::Asc))
            .set("StartIndex", paging.start_index);
        let list: dto::ItemList<dto::Playlist> = self.get(&self.items_path(), &params).await?;
        Ok(Page {
            total_count: list.total_record_count,
            items: list
                .items
                .into_iter()
                .map(|item| normalize::playlist(item, self.server_id()))
                .collect(),
            start_index: paging.start_index,
        })
    }

    async fn playlist_songs(&self, id: &str) -> Result<Vec<Song>> {
        let items = self.playlist_items(id, PLAYLIST_DETAIL_FIELDS).await?;
        Ok(self.songs(items))
    }

    async fn create_playlist(&self, name: &str, song_ids: &[String]) -> Result<Playlist> {
        let response = send(self.request(Method::POST, "playlists").json(&json!({
            "MediaType": "Audio",
            "Name": name,
            "UserId": self.user_id,
        })))
        .await?;
        let created: dto::CreatedPlaylist = decode(response).await?;
        self.add_to_playlist(&created.id, song_ids).await?;
        self.playlist_detail(&created.id).await
    }

    async fn add_to_playlist(&self, playlist_id: &str, song_ids: &[String]) -> Result<()> {
        let path = format!("playlists/{playlist_id}/items");
        for batch in song_ids.chunks(MAX_IDS_PER_REQUEST) {
            let params = Params::default()
                .set("Ids", batch.join(","))
                .set("UserId", &self.user_id);
            send(self.request(Method::POST, &path).query(&params.0)).await?;
        }
        Ok(())
    }

    async fn remove_from_playlist(&self, playlist_id: &str, indexes: &[usize]) -> Result<()> {
        if indexes.is_empty() {
            return Ok(());
        }
        let items = self.playlist_items(playlist_id, &[]).await?;
        let entry_ids = indexes
            .iter()
            .map(|index| {
                items
                    .get(*index)
                    .and_then(|item| item.playlist_item_id.clone())
                    .ok_or(ApiError::NotFound)
            })
            .collect::<Result<Vec<String>>>()?;
        let path = format!("playlists/{playlist_id}/items");
        for batch in entry_ids.chunks(MAX_IDS_PER_REQUEST) {
            let params = Params::default().set("EntryIds", batch.join(","));
            send(self.request(Method::DELETE, &path).query(&params.0)).await?;
        }
        Ok(())
    }

    async fn delete_playlist(&self, id: &str) -> Result<()> {
        send(self.request(Method::DELETE, &format!("items/{id}"))).await?;
        Ok(())
    }

    async fn search(&self, query: &str, limit: usize) -> Result<SearchResults> {
        if limit == 0 {
            return Ok(SearchResults {
                songs: Vec::new(),
                albums: Vec::new(),
                artists: Vec::new(),
            });
        }
        let album_params = Params::default()
            .set("EnableTotalRecordCount", true)
            .fields(ALBUM_LIST_FIELDS)
            .set("ImageTypeLimit", 1)
            .set("IncludeItemTypes", "MusicAlbum")
            .set("Limit", limit)
            .set("Recursive", true)
            .set("SearchTerm", query)
            .set("SortBy", "SortName")
            .set("SortOrder", sort_order(SortOrder::Asc))
            .set("StartIndex", 0);
        let albums: dto::ItemList<dto::Album> = self.get(&self.items_path(), &album_params).await?;

        let artist_params = Params::default()
            .set("EnableTotalRecordCount", true)
            .fields(ARTIST_LIST_FIELDS)
            .set("ImageTypeLimit", 1)
            .set("IncludeArtists", true)
            .set("Limit", limit)
            .set("Recursive", true)
            .set("SearchTerm", query)
            .set("StartIndex", 0)
            .set("UserId", &self.user_id);
        let artists = self.fetch_artists(&artist_params).await?;

        let song_params = Params::default()
            .set("EnableTotalRecordCount", true)
            .fields(SONG_FIELDS)
            .set("IncludeItemTypes", SONG_ITEM_TYPES)
            .set("Limit", limit)
            .set("Recursive", true)
            .set("SearchTerm", query)
            .set("SortBy", "Album,SortName")
            .set("SortOrder", sort_order(SortOrder::Asc))
            .set("StartIndex", 0)
            .set("UserId", &self.user_id);
        let songs: dto::ItemList<dto::Song> = self.get(&self.items_path(), &song_params).await?;

        Ok(SearchResults {
            songs: self.songs(songs.items),
            albums: albums
                .items
                .into_iter()
                .map(|item| normalize::album(item, self.server_id()))
                .collect(),
            artists: self.artists(artists.items),
        })
    }

    async fn set_favorite(&self, _kind: ItemKind, ids: &[String], favorite: bool) -> Result<()> {
        let method = if favorite {
            Method::POST
        } else {
            Method::DELETE
        };
        for id in ids {
            let path = format!("users/{}/favoriteitems/{id}", self.user_id);
            send(self.request(method.clone(), &path).json(&json!({}))).await?;
        }
        Ok(())
    }

    async fn set_rating(&self, _kind: ItemKind, _id: &str, _rating: u8) -> Result<()> {
        Err(ApiError::Unsupported("ratings"))
    }

    async fn scrobble(
        &self,
        song_id: &str,
        submission: bool,
        _timestamp_ms: Option<i64>,
    ) -> Result<()> {
        if !submission {
            return self
                .report(
                    "sessions/playing",
                    &dto::PlaybackReport {
                        item_id: song_id,
                        event_name: None,
                        is_paused: None,
                        position_ticks: Some(0),
                    },
                )
                .await;
        }
        let position_ticks = self
            .get::<dto::Song>(&self.item_path(song_id), &Params::default())
            .await
            .ok()
            .and_then(|song| song.run_time_ticks);
        self.report(
            "sessions/playing/stopped",
            &dto::PlaybackReport {
                item_id: song_id,
                event_name: None,
                is_paused: Some(true),
                position_ticks,
            },
        )
        .await
    }

    async fn save_play_queue(
        &self,
        _song_ids: &[String],
        _current_song_id: Option<&str>,
        _position_ms: u64,
    ) -> Result<()> {
        Err(ApiError::Unsupported("server play queue"))
    }

    async fn play_queue(&self) -> Result<Option<ServerPlayQueue>> {
        Err(ApiError::Unsupported("server play queue"))
    }

    async fn lyrics(&self, song: &Song) -> Result<Option<String>> {
        match self
            .get::<dto::Lyrics>(&format!("audio/{}/Lyrics", song.id), &Params::default())
            .await
        {
            Ok(lyrics) => Ok(normalize::lyrics(lyrics)),
            Err(ApiError::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn stream_url(&self, song: &Song, options: &StreamOptions) -> String {
        let (base, id, token) = (&self.base_url, &song.id, &self.access_token);
        if options.format.is_none() && options.max_bitrate_kbps.is_none() {
            return format!("{base}/Items/{id}/Download?apiKey={token}&playSessionId=");
        }
        let format = options.format.as_deref().unwrap_or("mp3");
        let session = format!("feishin-{id}-{}-0-0", format.to_lowercase());
        let mut url = format!(
            "{base}/audio/{id}/universal?userId={}&deviceId=&audioCodec={format}&apiKey={token}\
             &playSessionId={session}&container={format}&transcodingProtocol=http\
             &transcodingContainer={format}",
            self.user_id
        );
        if let Some(kbps) = options.max_bitrate_kbps {
            url.push_str(&format!("&maxStreamingBitrate={}", u64::from(kbps) * 1000));
        }
        url
    }

    fn cover_art_url(&self, cover_art_id: &str, size: Option<u32>) -> String {
        let mut url = format!(
            "{}/Items/{cover_art_id}/Images/Primary?quality=96",
            self.base_url
        );
        if let Some(size) = size {
            url.push_str(&format!("&fillHeight={size}&fillWidth={size}"));
        }
        url
    }
}
