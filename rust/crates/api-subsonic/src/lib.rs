//! Subsonic backend for `feishin_api::MusicServer` (port of `src/renderer/api/subsonic`).

mod client;
mod lyrics;
mod normalize;
mod sort;
mod types;

use std::future::Future;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use feishin_api::{ApiError, Credential, MusicServer, Result, StreamOptions};
use feishin_core::model::{
    Album, AlbumDetail, AlbumListQuery, AlbumSort, Artist, ArtistDetail, ArtistListQuery, Genre,
    ItemKind, MusicFolder, Page, Paging, Playlist, SearchResults, ServerConfig, ServerPlayQueue,
    Song, SongListQuery, SongSort, SortOrder,
};
use futures_util::stream::{self, StreamExt};
use serde::de::DeserializeOwned;
use tokio::sync::OnceCell;

use crate::client::{API_VERSION, CLIENT_NAME, Client, Params, encode};
use crate::types::{
    AlbumList2, Empty, Genres, GetAlbum, GetArtist, GetArtists, GetPlayQueue, GetPlayQueueByIndex,
    GetSong, LyricsList, MusicFolders, OpenSubsonicExtensions, Ping, PlayQueue, PlaylistResponse,
    Playlists, RandomSongs, Search3, SearchResult, SimilarSongs, SongsByGenre, SsAlbum, SsArtist,
    SsSong, Starred2, TopSongs,
};

const MAX_ITEMS: usize = 500;
const FAST_BATCH: usize = MAX_ITEMS * 10;
const CONCURRENCY: usize = 8;

/// OpenSubsonic extensions reported by `getOpenSubsonicExtensions` (all off for plain Subsonic).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Extensions {
    pub form_post: bool,
    pub index_based_queue: bool,
    pub playback_report: bool,
    pub top_songs_by_artist_id: bool,
    pub transcoding: bool,
    /// Versions of the `songLyrics` extension; empty when unsupported.
    pub song_lyrics: Vec<u32>,
}

impl Extensions {
    fn from_response(response: OpenSubsonicExtensions) -> Self {
        let mut ext = Self::default();
        for e in response.open_subsonic_extensions.unwrap_or_default() {
            match e.name.as_str() {
                "formPost" => ext.form_post = true,
                "indexBasedQueue" => ext.index_based_queue = true,
                "playbackReport" => ext.playback_report = true,
                "topSongsByArtistId" => ext.top_songs_by_artist_id = true,
                "transcoding" => ext.transcoding = true,
                "songLyrics" => ext.song_lyrics = e.versions,
                _ => {}
            }
        }
        ext
    }
}

pub struct SubsonicServer {
    config: ServerConfig,
    client: Client,
    extensions: OnceCell<Extensions>,
}

impl SubsonicServer {
    /// Token auth (`t = md5(password + salt)`), retried with a plain `p` password when the server rejects it.
    pub async fn login(config: ServerConfig, password: &str) -> Result<(Self, Credential)> {
        let token = client::token_credential(&config.username, password);
        let server = Self::new(config, token)?;
        let (server, credential) = match server.authenticate().await {
            Ok(()) => {
                let credential = server.client.auth().to_owned();
                (server, credential)
            }
            Err(ApiError::Auth(_)) => {
                let legacy = client::password_credential(&server.config.username, password);
                let server = Self::new(server.config, legacy.clone())?;
                server.authenticate().await?;
                (server, legacy)
            }
            Err(e) => return Err(e),
        };
        Ok((
            server,
            Credential::Subsonic {
                subsonic: credential,
            },
        ))
    }

    pub fn with_credential(config: ServerConfig, credential: &Credential) -> Result<Self> {
        match credential {
            Credential::Subsonic { subsonic } => {
                client::validate_credential(subsonic)?;
                Self::new(config, subsonic.clone())
            }
            _ => Err(ApiError::Auth("expected a subsonic credential".into())),
        }
    }

    fn new(config: ServerConfig, auth: String) -> Result<Self> {
        let client = Client::new(&config.url, auth)?;
        Ok(Self {
            config,
            client,
            extensions: OnceCell::new(),
        })
    }

    async fn authenticate(&self) -> Result<()> {
        self.client
            .call(
                "getUser.view",
                &Params::new().add("username", &self.config.username),
                false,
            )
            .await
            .map(drop)
    }

    /// Detected once per session via `ping` + `getOpenSubsonicExtensions`, like the TS `getServerInfo`.
    pub async fn extensions(&self) -> Result<&Extensions> {
        self.extensions
            .get_or_try_init(|| async {
                let ping: Ping = self
                    .client
                    .call("ping.view", &Params::new(), false)
                    .await?
                    .decode()?;
                if !ping.open_subsonic.unwrap_or(false) || ping.server_version.is_none() {
                    return Ok(Extensions::default());
                }
                let response: OpenSubsonicExtensions = self
                    .client
                    .call("getOpenSubsonicExtensions.view", &Params::new(), false)
                    .await?
                    .decode()?;
                Ok(Extensions::from_response(response))
            })
            .await
    }

    async fn fetch<T: DeserializeOwned>(&self, endpoint: &str, params: Params) -> Result<T> {
        self.fetch_with_total(endpoint, params)
            .await
            .map(|(v, _)| v)
    }

    async fn fetch_with_total<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        params: Params,
    ) -> Result<(T, Option<usize>)> {
        let form_post = self.extensions.get().is_some_and(|e| e.form_post);
        let payload = self.client.call(endpoint, &params, form_post).await?;
        let total = payload.total_count;
        Ok((payload.decode()?, total))
    }

    async fn send(&self, endpoint: &str, params: Params) -> Result<()> {
        self.fetch::<Empty>(endpoint, params).await.map(drop)
    }

    fn sid(&self) -> &str {
        &self.config.id
    }

    fn songs(&self, items: Option<Vec<SsSong>>) -> Vec<Song> {
        items
            .unwrap_or_default()
            .into_iter()
            .map(|s| normalize::song(s, self.sid()))
            .collect()
    }

    fn albums(&self, items: Option<Vec<SsAlbum>>) -> Vec<Album> {
        items
            .unwrap_or_default()
            .into_iter()
            .map(|a| normalize::album_only(a, self.sid()))
            .collect()
    }

    fn artists(&self, items: Option<Vec<SsArtist>>) -> Vec<Artist> {
        items
            .unwrap_or_default()
            .into_iter()
            .map(|a| normalize::artist(a, self.sid()))
            .collect()
    }

    async fn search3(
        &self,
        query: &str,
        counts: [usize; 3],
        offsets: [usize; 3],
        folder: Option<&str>,
    ) -> Result<SearchResult> {
        let params = Params::new()
            .add("query", query)
            .add("artistCount", counts[0])
            .add("artistOffset", offsets[0])
            .add("albumCount", counts[1])
            .add("albumOffset", offsets[1])
            .add("songCount", counts[2])
            .add("songOffset", offsets[2])
            .opt("musicFolderId", folder);
        let res: Search3 = self.fetch("search3.view", params).await?;
        Ok(res.search_result3.unwrap_or_default())
    }

    async fn search_songs(
        &self,
        query: &str,
        offset: usize,
        count: usize,
        folder: Option<&str>,
    ) -> Result<Vec<SsSong>> {
        let res = self
            .search3(query, [0, 0, count], [0, 0, offset], folder)
            .await?;
        Ok(res.song.unwrap_or_default())
    }

    async fn search_albums(
        &self,
        query: &str,
        offset: usize,
        count: usize,
        folder: Option<&str>,
    ) -> Result<Vec<SsAlbum>> {
        let res = self
            .search3(query, [0, count, 0], [0, offset, 0], folder)
            .await?;
        Ok(res.album.unwrap_or_default())
    }

    async fn songs_by_genre(
        &self,
        genre: &str,
        offset: usize,
        count: usize,
        folder: Option<&str>,
    ) -> Result<Vec<SsSong>> {
        let params = Params::new()
            .add("genre", genre)
            .add("count", count)
            .add("offset", offset)
            .opt("musicFolderId", folder);
        let res: SongsByGenre = self.fetch("getSongsByGenre.view", params).await?;
        Ok(res.songs_by_genre.and_then(|c| c.song).unwrap_or_default())
    }

    async fn starred(&self, folder: Option<&str>) -> Result<SearchResult> {
        let res: Starred2 = self
            .fetch(
                "getStarred2.view",
                Params::new().opt("musicFolderId", folder),
            )
            .await?;
        Ok(res.starred2.unwrap_or_default())
    }

    async fn starred_songs(&self, query: &SongListQuery) -> Result<Vec<Song>> {
        let starred = self.starred(query.music_folder_id.as_deref()).await?;
        let mut songs = self.songs(starred.song);
        if !query.artist_ids.is_empty() {
            songs.retain(|s| {
                s.album_artists
                    .iter()
                    .any(|a| query.artist_ids.contains(&a.id))
            });
        }
        Ok(songs)
    }

    /// Fetches `endpoint?id=` for each id with bounded concurrency, keeping order. Items the server
    /// rejects are skipped, like the TS `Promise.all` + `status !== 200` filtering.
    async fn fetch_each<T: DeserializeOwned>(
        &self,
        endpoint: &'static str,
        ids: &[String],
    ) -> Result<Vec<T>> {
        let requests: Vec<_> = ids
            .iter()
            .map(|id| self.fetch::<T>(endpoint, Params::new().add("id", id)))
            .collect();
        let results: Vec<Result<T>> = stream::iter(requests).buffered(CONCURRENCY).collect().await;
        let mut out = Vec::with_capacity(results.len());
        for result in results {
            match result {
                Ok(v) => out.push(v),
                Err(ApiError::Server { .. } | ApiError::NotFound) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }

    async fn artist_albums(&self, artist_ids: &[String]) -> Result<Vec<SsAlbum>> {
        let artists: Vec<GetArtist> = self.fetch_each("getArtist.view", artist_ids).await?;
        Ok(artists
            .into_iter()
            .flat_map(|a| a.artist.album.unwrap_or_default())
            .collect())
    }

    async fn songs_of(&self, album_ids: &[String], artist_ids: &[String]) -> Result<Vec<SsSong>> {
        let mut ids = album_ids.to_vec();
        if !artist_ids.is_empty() {
            ids.extend(
                self.artist_albums(artist_ids)
                    .await?
                    .into_iter()
                    .map(|a| a.id),
            );
        }
        let albums: Vec<GetAlbum> = self.fetch_each("getAlbum.view", &ids).await?;
        Ok(albums
            .into_iter()
            .flat_map(|a| a.album.song.unwrap_or_default())
            .collect())
    }

    async fn artist_index(&self, folder: Option<&str>) -> Result<Vec<SsArtist>> {
        let res: GetArtists = self
            .fetch(
                "getArtists.view",
                Params::new().opt("musicFolderId", folder),
            )
            .await?;
        Ok(res
            .artists
            .and_then(|a| a.index)
            .unwrap_or_default()
            .into_iter()
            .flat_map(|i| i.artist.unwrap_or_default())
            .collect())
    }

    async fn playlist_entries(&self, id: &str) -> Result<Vec<SsSong>> {
        let res: PlaylistResponse = self
            .fetch("getPlaylist.view", Params::new().add("id", id))
            .await?;
        let playlist = res.playlist.ok_or(ApiError::NotFound)?;
        Ok(playlist.entry.unwrap_or_default())
    }

    fn play_queue_from(&self, queue: PlayQueue, by_index: bool) -> ServerPlayQueue {
        let songs = self.songs(queue.entry);
        let current_song_id = if by_index {
            queue
                .current_index
                .filter(|i| *i >= 0.0)
                .and_then(|i| songs.get(i as usize))
                .map(|s| s.id.clone())
        } else {
            queue.current
        };
        ServerPlayQueue {
            songs,
            current_song_id,
            position_ms: queue
                .position
                .filter(|p| p.is_finite() && *p > 0.0)
                .map_or(0, |p| p as u64),
        }
    }

    fn url(&self, endpoint: &str, id: &str) -> String {
        format!("{}/rest/{endpoint}?id={}", self.client.base(), encode(id))
    }
}

fn non_empty(value: &Option<String>) -> Option<&str> {
    value.as_deref().filter(|s| !s.is_empty())
}

fn nonzero(value: Option<i32>) -> Option<i32> {
    value.filter(|v| *v != 0)
}

/// Pages through `MAX_ITEMS` at a time starting at `start`; returns the total.
async fn paged_count<F, Fut>(mut start: usize, fetch: F) -> Result<usize>
where
    F: Fn(usize, usize) -> Fut,
    Fut: Future<Output = Result<usize>>,
{
    loop {
        let n = fetch(start, MAX_ITEMS).await?;
        start += n;
        if n != MAX_ITEMS {
            return Ok(start);
        }
    }
}

/// Probes single items every `FAST_BATCH` offsets, then pages from the last full section
/// (same strategy as the TS `getSongListCount`).
async fn fast_count<F, Fut>(fetch: F) -> Result<usize>
where
    F: Fn(usize, usize) -> Fut,
    Fut: Future<Output = Result<usize>>,
{
    let mut section = 0;
    while fetch(section, 1).await? == 1 {
        section += FAST_BATCH;
    }
    paged_count(section.saturating_sub(FAST_BATCH), fetch).await
}

fn current_year() -> i32 {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    i32::try_from(year).unwrap_or(i32::MAX)
}

fn album_list_type(sort: AlbumSort) -> &'static str {
    match sort {
        AlbumSort::AlbumArtist => "alphabeticalByArtist",
        AlbumSort::Favorited => "starred",
        AlbumSort::PlayCount => "frequent",
        AlbumSort::Random => "random",
        AlbumSort::RecentlyAdded => "newest",
        AlbumSort::RecentlyPlayed => "recent",
        AlbumSort::Year => "byYear",
        AlbumSort::Name | AlbumSort::Rating | AlbumSort::SongCount | AlbumSort::Duration => {
            "alphabeticalByName"
        }
    }
}

/// `getAlbumList2` parameters (without paging) mirroring the TS genre/year handling.
fn album_list_params(query: &AlbumListQuery, base_type: &'static str) -> Params {
    let min_year = nonzero(query.year_from);
    let max_year = nonzero(query.year_to);
    let mut list_type = base_type;
    if !query.genre_ids.is_empty() {
        list_type = "byGenre";
    }
    if min_year.is_some() || max_year.is_some() {
        list_type = "byYear";
    }

    let mut from_year = None;
    let mut to_year = None;
    if let Some(min) = min_year {
        from_year = Some(min);
        to_year = Some(current_year());
    }
    if let Some(max) = max_year {
        to_year = Some(max);
        if min_year.is_none() {
            from_year = Some(0);
        }
    }
    if list_type == "byYear" && from_year.is_none() && to_year.is_none() {
        if query.order == SortOrder::Asc {
            from_year = Some(0);
            to_year = Some(current_year());
        } else {
            from_year = Some(current_year());
            to_year = Some(0);
        }
    }

    Params::new()
        .add("type", list_type)
        .opt("genre", query.genre_ids.first())
        .opt("fromYear", from_year)
        .opt("toYear", to_year)
        .opt("musicFolderId", query.music_folder_id.as_deref())
}

#[async_trait]
impl MusicServer for SubsonicServer {
    fn config(&self) -> &ServerConfig {
        &self.config
    }

    async fn ping(&self) -> Result<()> {
        self.send("ping.view", Params::new()).await
    }

    async fn song_list(&self, query: &SongListQuery, paging: Paging) -> Result<Page<Song>> {
        let folder = query.music_folder_id.as_deref();
        let start = paging.start_index;
        let page = |items: Vec<Song>, total_count: Option<usize>| Page {
            items,
            start_index: start,
            total_count,
        };

        if let Some(term) = non_empty(&query.search) {
            let songs = self.search_songs(term, start, paging.limit, folder).await?;
            return Ok(page(self.songs(Some(songs)), None));
        }

        let by_ids = !query.album_ids.is_empty() || !query.artist_ids.is_empty();
        if query.sort == SongSort::Random && query.favorite != Some(true) && !by_ids {
            let params = Params::new()
                .add("size", paging.limit)
                .opt("genre", query.genre_ids.first())
                .opt("fromYear", nonzero(query.year_from))
                .opt("toYear", nonzero(query.year_to))
                .opt("musicFolderId", folder);
            let res: RandomSongs = self.fetch("getRandomSongs.view", params).await?;
            return Ok(page(
                self.songs(res.random_songs.and_then(|c| c.song)),
                None,
            ));
        }

        if let Some(genre) = query.genre_ids.first() {
            let songs = self
                .songs_by_genre(genre, start, paging.limit, folder)
                .await?;
            return Ok(page(self.songs(Some(songs)), None));
        }

        if query.favorite == Some(true) {
            let mut songs = self.starred_songs(query).await?;
            sort::songs(&mut songs, query.sort, query.order);
            let total = songs.len();
            return Ok(page(
                sort::paginate(songs, start, paging.limit),
                Some(total),
            ));
        }

        if by_ids {
            let songs = self.songs_of(&query.album_ids, &query.artist_ids).await?;
            let songs = self.songs(Some(songs));
            let total = songs.len();
            return Ok(page(
                sort::paginate(songs, start, paging.limit),
                Some(total),
            ));
        }

        let songs = self.search_songs("", start, paging.limit, folder).await?;
        Ok(page(self.songs(Some(songs)), None))
    }

    async fn song_count(&self, query: &SongListQuery) -> Result<usize> {
        let folder = query.music_folder_id.as_deref();

        if let Some(term) = non_empty(&query.search) {
            return paged_count(0, |offset, count| async move {
                Ok(self.search_songs(term, offset, count, folder).await?.len())
            })
            .await;
        }

        if let Some(genre) = query.genre_ids.first() {
            return fast_count(|offset, count| async move {
                Ok(self
                    .songs_by_genre(genre, offset, count, folder)
                    .await?
                    .len())
            })
            .await;
        }

        if query.favorite == Some(true) {
            return Ok(self.starred_songs(query).await?.len());
        }

        if !query.album_ids.is_empty() || !query.artist_ids.is_empty() {
            return Ok(self
                .songs_of(&query.album_ids, &query.artist_ids)
                .await?
                .len());
        }

        fast_count(|offset, count| async move {
            Ok(self.search_songs("", offset, count, folder).await?.len())
        })
        .await
    }

    async fn song(&self, id: &str) -> Result<Song> {
        let res: GetSong = self
            .fetch("getSong.view", Params::new().add("id", id))
            .await?;
        Ok(normalize::song(res.song, self.sid()))
    }

    async fn random_songs(&self, count: usize, genre: Option<&str>) -> Result<Vec<Song>> {
        let params = Params::new().add("size", count).opt("genre", genre);
        let res: RandomSongs = self.fetch("getRandomSongs.view", params).await?;
        Ok(self.songs(res.random_songs.and_then(|c| c.song)))
    }

    async fn similar_songs(&self, song_id: &str, count: usize) -> Result<Vec<Song>> {
        let params = Params::new().add("id", song_id).add("count", count);
        let res: SimilarSongs = self.fetch("getSimilarSongs.view", params).await?;
        let mut songs = self.songs(res.similar_songs.and_then(|c| c.song));
        songs.retain(|s| s.id != song_id);
        Ok(songs)
    }

    async fn album_list(&self, query: &AlbumListQuery, paging: Paging) -> Result<Page<Album>> {
        let folder = query.music_folder_id.as_deref();
        let start = paging.start_index;

        if let Some(term) = non_empty(&query.search) {
            let albums = self
                .search_albums(term, start, paging.limit, folder)
                .await?;
            return Ok(Page {
                items: self.albums(Some(albums)),
                start_index: start,
                total_count: None,
            });
        }

        let client_side = if !query.artist_ids.is_empty() {
            Some(self.artist_albums(&query.artist_ids).await?)
        } else if query.favorite == Some(true) {
            Some(self.starred(folder).await?.album.unwrap_or_default())
        } else {
            None
        };
        if let Some(albums) = client_side {
            let mut albums = self.albums(Some(albums));
            sort::albums(&mut albums, query.sort, query.order);
            let total = albums.len();
            return Ok(Page {
                items: sort::paginate(albums, start, paging.limit),
                start_index: start,
                total_count: Some(total),
            });
        }

        let params = album_list_params(query, album_list_type(query.sort))
            .add("size", paging.limit)
            .add("offset", start);
        let (res, total): (AlbumList2, _) =
            self.fetch_with_total("getAlbumList2.view", params).await?;
        Ok(Page {
            items: self.albums(res.album_list2.and_then(|c| c.album)),
            start_index: start,
            total_count: total,
        })
    }

    async fn album_count(&self, query: &AlbumListQuery) -> Result<usize> {
        let folder = query.music_folder_id.as_deref();

        if let Some(term) = non_empty(&query.search) {
            return paged_count(0, |offset, count| async move {
                Ok(self.search_albums(term, offset, count, folder).await?.len())
            })
            .await;
        }

        if !query.artist_ids.is_empty() {
            return Ok(self.artist_albums(&query.artist_ids).await?.len());
        }

        if query.favorite == Some(true) {
            return Ok(self.starred(folder).await?.album.map_or(0, |a| a.len()));
        }

        let base = album_list_params(query, "alphabeticalByName");
        let mut start = 0;
        loop {
            let params = base.clone().add("size", MAX_ITEMS).add("offset", start);
            let (res, total): (AlbumList2, _) =
                self.fetch_with_total("getAlbumList2.view", params).await?;
            if let Some(total) = total {
                return Ok(total);
            }
            let n = res.album_list2.and_then(|c| c.album).map_or(0, |a| a.len());
            start += n;
            if n != MAX_ITEMS {
                return Ok(start);
            }
        }
    }

    async fn album(&self, id: &str) -> Result<AlbumDetail> {
        let res: GetAlbum = self
            .fetch("getAlbum.view", Params::new().add("id", id))
            .await?;
        let (album, songs) = normalize::album(res.album, self.sid());
        Ok(AlbumDetail { album, songs })
    }

    async fn artist_list(&self, query: &ArtistListQuery, paging: Paging) -> Result<Page<Artist>> {
        let mut artists = self.artist_index(query.music_folder_id.as_deref()).await?;
        if let Some(term) = non_empty(&query.search) {
            let term = term.to_lowercase();
            artists.retain(|a| a.name.to_lowercase().contains(&term));
        }
        if query.favorite == Some(true) {
            artists.retain(|a| normalize::truthy(&a.starred));
        }
        sort::artists(&mut artists, query.sort, query.order);
        let total = artists.len();
        Ok(Page {
            items: self.artists(Some(sort::paginate(
                artists,
                paging.start_index,
                paging.limit,
            ))),
            start_index: paging.start_index,
            total_count: Some(total),
        })
    }

    async fn artist(&self, id: &str) -> Result<ArtistDetail> {
        let res: GetArtist = self
            .fetch("getArtist.view", Params::new().add("id", id))
            .await?;
        let mut raw = res.artist;
        let albums = self.albums(raw.album.take());

        let by_id = self
            .extensions()
            .await
            .is_ok_and(|e| e.top_songs_by_artist_id);
        let top_params = if by_id {
            Params::new().add("id", id)
        } else {
            Params::new().add("artist", &raw.name)
        };
        let (top, info) = futures_util::future::join(
            self.fetch::<TopSongs>("getTopSongs.view", top_params),
            self.fetch::<types::ArtistInfo2>("getArtistInfo2.view", Params::new().add("id", id)),
        )
        .await;

        let mut artist = normalize::artist(raw, self.sid());
        artist.biography = info
            .ok()
            .and_then(|i| i.artist_info2)
            .and_then(|i| i.biography)
            .filter(|b| !b.is_empty());
        let top_songs = top
            .map(|t| self.songs(t.top_songs.and_then(|c| c.song)))
            .unwrap_or_default();
        Ok(ArtistDetail {
            artist,
            albums,
            top_songs,
        })
    }

    async fn genre_list(&self) -> Result<Vec<Genre>> {
        let res: Genres = self.fetch("getGenres.view", Params::new()).await?;
        let mut genres: Vec<Genre> = res
            .genres
            .and_then(|g| g.genre)
            .unwrap_or_default()
            .into_iter()
            .map(normalize::genre)
            .collect();
        genres.sort_by_cached_key(|g| g.name.to_lowercase());
        Ok(genres)
    }

    async fn music_folders(&self) -> Result<Vec<MusicFolder>> {
        let res: MusicFolders = self.fetch("getMusicFolders.view", Params::new()).await?;
        Ok(res
            .music_folders
            .and_then(|m| m.music_folder)
            .unwrap_or_default()
            .into_iter()
            .map(normalize::music_folder)
            .collect())
    }

    async fn playlist_list(&self, paging: Paging) -> Result<Page<Playlist>> {
        let res: Playlists = self.fetch("getPlaylists.view", Params::new()).await?;
        let playlists: Vec<Playlist> = res
            .playlists
            .and_then(|p| p.playlist)
            .unwrap_or_default()
            .into_iter()
            .map(|p| normalize::playlist(p, self.sid()))
            .collect();
        let total = playlists.len();
        Ok(Page {
            items: sort::paginate(playlists, paging.start_index, paging.limit),
            start_index: paging.start_index,
            total_count: Some(total),
        })
    }

    async fn playlist_songs(&self, id: &str) -> Result<Vec<Song>> {
        let entries = self.playlist_entries(id).await?;
        Ok(self.songs(Some(entries)))
    }

    async fn create_playlist(&self, name: &str, song_ids: &[String]) -> Result<Playlist> {
        let params = Params::new().add("name", name).many("songId", song_ids);
        let res: PlaylistResponse = self.fetch("createPlaylist.view", params).await?;
        res.playlist
            .map(|p| normalize::playlist(p, self.sid()))
            .ok_or_else(|| ApiError::Decode("createPlaylist returned no playlist".into()))
    }

    async fn add_to_playlist(&self, playlist_id: &str, song_ids: &[String]) -> Result<()> {
        let params = Params::new()
            .add("playlistId", playlist_id)
            .many("songIdToAdd", song_ids);
        self.send("updatePlaylist.view", params).await
    }

    async fn remove_from_playlist(&self, playlist_id: &str, indexes: &[usize]) -> Result<()> {
        let indexes: Vec<String> = indexes.iter().map(ToString::to_string).collect();
        let params = Params::new()
            .add("playlistId", playlist_id)
            .many("songIndexToRemove", &indexes);
        self.send("updatePlaylist.view", params).await
    }

    async fn delete_playlist(&self, id: &str) -> Result<()> {
        self.send("deletePlaylist.view", Params::new().add("id", id))
            .await
    }

    async fn search(&self, query: &str, limit: usize) -> Result<SearchResults> {
        let res = self.search3(query, [limit; 3], [0; 3], None).await?;
        Ok(SearchResults {
            songs: self.songs(res.song),
            albums: self.albums(res.album),
            artists: self.artists(res.artist),
        })
    }

    async fn set_favorite(&self, kind: ItemKind, ids: &[String], favorite: bool) -> Result<()> {
        let key = match kind {
            ItemKind::Song => "id",
            ItemKind::Album => "albumId",
            ItemKind::Artist => "artistId",
            ItemKind::Playlist => return Err(ApiError::Unsupported("favorite playlists")),
        };
        if ids.is_empty() {
            return Ok(());
        }
        let endpoint = if favorite { "star.view" } else { "unstar.view" };
        self.send(endpoint, Params::new().many(key, ids)).await
    }

    async fn set_rating(&self, kind: ItemKind, id: &str, rating: u8) -> Result<()> {
        if kind == ItemKind::Playlist {
            return Err(ApiError::Unsupported("rating playlists"));
        }
        let params = Params::new().add("id", id).add("rating", rating.min(5));
        self.send("setRating.view", params).await
    }

    async fn scrobble(
        &self,
        song_id: &str,
        submission: bool,
        timestamp_ms: Option<i64>,
    ) -> Result<()> {
        let params = Params::new()
            .add("id", song_id)
            .add("submission", submission)
            .opt("time", timestamp_ms.filter(|_| submission));
        self.send("scrobble.view", params).await
    }

    async fn save_play_queue(
        &self,
        song_ids: &[String],
        current_song_id: Option<&str>,
        position_ms: u64,
    ) -> Result<()> {
        let current_index = current_song_id.and_then(|id| song_ids.iter().position(|s| s == id));
        let params = Params::new()
            .many("id", song_ids)
            .add("position", position_ms);
        if self.extensions().await?.index_based_queue {
            self.send(
                "savePlayQueueByIndex.view",
                params.opt("currentIndex", current_index),
            )
            .await
        } else {
            let current = current_index.and_then(|i| song_ids.get(i));
            self.send("savePlayQueue.view", params.opt("current", current))
                .await
        }
    }

    async fn play_queue(&self) -> Result<Option<ServerPlayQueue>> {
        let by_index = self.extensions().await?.index_based_queue;
        let queue = if by_index {
            self.fetch::<GetPlayQueueByIndex>("getPlayQueueByIndex.view", Params::new())
                .await
                .map(|r| r.play_queue_by_index)
        } else {
            self.fetch::<GetPlayQueue>("getPlayQueue.view", Params::new())
                .await
                .map(|r| r.play_queue)
        };
        match queue {
            Ok(queue) => Ok(queue.map(|q| self.play_queue_from(q, by_index))),
            Err(ApiError::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn lyrics(&self, song: &Song) -> Result<Option<String>> {
        let versions = &self.extensions().await?.song_lyrics;
        if versions.is_empty() {
            return Ok(None);
        }
        let params = Params::new()
            .add("id", &song.id)
            .opt("enhanced", versions.contains(&2).then_some(true));
        let res: LyricsList = match self.fetch("getLyricsBySongId.view", params).await {
            Ok(res) => res,
            Err(ApiError::NotFound) => return Ok(None),
            Err(e) => return Err(e),
        };
        let lyrics = res
            .lyrics_list
            .and_then(|l| l.structured_lyrics)
            .unwrap_or_default();
        Ok(lyrics::to_text(&lyrics))
    }

    fn stream_url(&self, song: &Song, options: &StreamOptions) -> String {
        let mut url = format!(
            "{}&v={API_VERSION}&c={CLIENT_NAME}&{}",
            self.url("stream.view", &song.id),
            self.client.auth()
        );
        if let Some(format) = options.format.as_deref().filter(|f| !f.is_empty()) {
            url.push_str("&format=");
            url.push_str(&encode(format));
        }
        if let Some(bitrate) = options.max_bitrate_kbps {
            url.push_str(&format!("&maxBitRate={bitrate}"));
        }
        url
    }

    fn cover_art_url(&self, cover_art_id: &str, size: Option<u32>) -> String {
        let mut url = format!(
            "{}&{}&v={API_VERSION}&c={CLIENT_NAME}",
            self.url("getCoverArt.view", cover_art_id),
            self.client.auth()
        );
        if let Some(size) = size.filter(|s| *s > 0) {
            url.push_str(&format!("&size={size}"));
        }
        url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_year_is_plausible() {
        let year = current_year();
        assert!((2024..3000).contains(&year));
    }

    #[test]
    fn year_filter_switches_to_by_year() {
        let query = AlbumListQuery {
            year_to: Some(1990),
            ..Default::default()
        };
        let params = format!("{:?}", album_list_params(&query, "newest"));
        assert!(params.contains("\"byYear\""));
        assert!(params.contains("(\"fromYear\", \"0\")"));
        assert!(params.contains("(\"toYear\", \"1990\")"));
    }

    #[test]
    fn year_sort_without_range_uses_order() {
        let query = AlbumListQuery {
            sort: AlbumSort::Year,
            order: SortOrder::Desc,
            ..Default::default()
        };
        let params = format!(
            "{:?}",
            album_list_params(&query, album_list_type(query.sort))
        );
        assert!(params.contains("(\"toYear\", \"0\")"));
        assert!(params.contains(&format!("(\"fromYear\", \"{}\")", current_year())));
    }
}
