//! In-process fake server for `--demo` and tests: a generated library computed on demand, so it costs
//! almost no memory regardless of size.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use feishin_api::{ApiError, Credential, MusicServer, Result, StreamOptions};
use feishin_core::model::{
    Album, AlbumDetail, AlbumListQuery, Artist, ArtistDetail, ArtistListQuery, ArtistRef, Genre,
    ItemKind, MusicFolder, Page, Paging, Playlist, SearchResults, ServerConfig, ServerPlayQueue,
    ServerType, Song, SongListQuery, SongSort, SortOrder,
};

pub const DEMO_SONG_COUNT: usize = 20_000;
const SONGS_PER_ALBUM: usize = 12;
const ARTIST_COUNT: usize = 400;

const ADJECTIVES: [&str; 24] = [
    "Silent", "Golden", "Electric", "Broken", "Midnight", "Velvet", "Crimson", "Hollow", "Neon",
    "Wild", "Paper", "Frozen", "Burning", "Distant", "Secret", "Lunar", "Restless", "Glass",
    "Static", "Summer", "Faded", "Copper", "Endless", "Quiet",
];
const NOUNS: [&str; 24] = [
    "River",
    "Heart",
    "City",
    "Signal",
    "Garden",
    "Mirror",
    "Engine",
    "Ocean",
    "Highway",
    "Dream",
    "Echo",
    "Machine",
    "Forest",
    "Window",
    "Satellite",
    "Shadow",
    "Harbor",
    "Letter",
    "Fire",
    "Horizon",
    "Station",
    "Island",
    "Thunder",
    "Season",
];
const FIRST_NAMES: [&str; 20] = [
    "Ada", "Bruno", "Clara", "Dmitri", "Elena", "Felix", "Greta", "Hugo", "Iris", "Jonas", "Kira",
    "Luca", "Mira", "Nils", "Olga", "Pavel", "Rosa", "Sami", "Tove", "Yuri",
];
const LAST_NAMES: [&str; 20] = [
    "Vale", "Stone", "Moreau", "Kade", "Lind", "Okafor", "Sato", "Brandt", "Costa", "Novak",
    "Reyes", "Holm", "Ferrari", "Quinn", "Ivers", "Nakamura", "Duarte", "Weiss", "Arden", "Bloom",
];
const GENRES: [&str; 8] = [
    "Rock",
    "Electronic",
    "Jazz",
    "Hip-Hop",
    "Ambient",
    "Folk",
    "Classical",
    "Pop",
];

fn mix(value: u64) -> u64 {
    let mut z = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

fn pick<'a>(words: &[&'a str], hash: u64) -> &'a str {
    words[(hash % words.len() as u64) as usize]
}

fn parse_index(id: &str, prefix: &str) -> Option<usize> {
    id.strip_prefix(prefix)?.parse().ok()
}

/// A song with only an id, title and duration set.
pub fn blank_song(id: &str, server_id: &str, duration_ms: u64) -> Song {
    Song {
        id: id.to_owned(),
        server_id: server_id.to_owned(),
        title: id.to_owned(),
        artists: Vec::new(),
        album_artists: Vec::new(),
        album: None,
        album_id: None,
        duration_ms,
        track_number: None,
        disc_number: None,
        year: None,
        genres: Vec::new(),
        bitrate_kbps: None,
        play_count: 0,
        user_rating: None,
        favorite: false,
        cover_art_id: None,
        replay_gain: None,
        path: None,
        size_bytes: None,
        container: None,
        created_at: None,
        last_played_at: None,
    }
}

#[cfg(test)]
pub fn test_song(id: &str, duration_ms: u64) -> Song {
    blank_song(id, "demo", duration_ms)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrobbleCall {
    pub song_id: String,
    pub submission: bool,
    pub timestamp_ms: Option<i64>,
}

pub struct FakeServer {
    config: ServerConfig,
    song_count: usize,
    order_cache: Mutex<Option<(SongListQuery, Arc<Vec<usize>>)>>,
    scrobbles: Mutex<Vec<ScrobbleCall>>,
    playlists: Mutex<DemoPlaylists>,
}

struct DemoPlaylist {
    id: String,
    name: String,
    comment: Option<String>,
    songs: Vec<usize>,
}

#[derive(Default)]
struct DemoPlaylists {
    items: Vec<DemoPlaylist>,
    next_id: usize,
}

impl DemoPlaylists {
    const NAMES: [&str; 6] = [
        "Morning Coffee",
        "Workout",
        "Late Night Drive",
        "Focus",
        "Road Trip",
        "Rainy Day",
    ];

    fn generated(song_count: usize) -> Self {
        let mut playlists = Self::default();
        if song_count == 0 {
            return playlists;
        }
        for (p, name) in Self::NAMES.iter().enumerate() {
            let len = 8 + (mix(p as u64 ^ 0x9e7) % 60) as usize;
            let songs = (0..len)
                .map(|i| (mix((p * 1000 + i) as u64 ^ 0x91) % song_count as u64) as usize)
                .collect();
            let comment = (p % 2 == 0).then(|| format!("A demo playlist with {len} songs"));
            playlists.push(name, comment, songs);
        }
        playlists
    }

    fn push(&mut self, name: &str, comment: Option<String>, songs: Vec<usize>) -> &DemoPlaylist {
        self.next_id += 1;
        self.items.push(DemoPlaylist {
            id: format!("playlist-{}", self.next_id),
            name: name.to_owned(),
            comment,
            songs,
        });
        &self.items[self.items.len() - 1]
    }

    fn get_mut(&mut self, id: &str) -> Result<&mut DemoPlaylist> {
        self.items
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or(ApiError::NotFound)
    }
}

impl FakeServer {
    pub fn config_for_demo() -> ServerConfig {
        ServerConfig {
            id: "demo".into(),
            name: "Demo library".into(),
            server_type: ServerType::Navidrome,
            url: "demo://library".into(),
            username: "demo".into(),
        }
    }

    #[cfg(test)]
    pub fn new(song_count: usize) -> Self {
        Self::with_config(Self::config_for_demo(), song_count)
    }

    pub fn with_config(config: ServerConfig, song_count: usize) -> Self {
        Self {
            config,
            song_count,
            order_cache: Mutex::default(),
            scrobbles: Mutex::default(),
            playlists: Mutex::new(DemoPlaylists::generated(song_count)),
        }
    }

    #[cfg(test)]
    pub fn scrobbles(&self) -> Vec<ScrobbleCall> {
        self.scrobbles
            .lock()
            .map(|calls| calls.clone())
            .unwrap_or_default()
    }

    fn album_count(&self) -> usize {
        self.song_count.div_ceil(SONGS_PER_ALBUM)
    }

    fn artist_name(artist: usize) -> String {
        let hash = mix(artist as u64 ^ 0xa11);
        format!(
            "{} {}",
            pick(&FIRST_NAMES, hash),
            pick(&LAST_NAMES, hash >> 16)
        )
    }

    fn artist_ref(artist: usize) -> ArtistRef {
        ArtistRef {
            id: format!("artist-{artist}"),
            name: Self::artist_name(artist),
        }
    }

    fn album_name(album: usize) -> String {
        let hash = mix(album as u64 ^ 0xa1b);
        format!("{} {}", pick(&ADJECTIVES, hash), pick(&NOUNS, hash >> 16))
    }

    fn album_artist(album: usize) -> usize {
        (mix(album as u64) % ARTIST_COUNT as u64) as usize
    }

    fn album_year(album: usize) -> i32 {
        1965 + (mix(album as u64 ^ 0x7ea) % 60) as i32
    }

    fn song_title(index: usize) -> String {
        let hash = mix(index as u64);
        let title = format!("{} {}", pick(&ADJECTIVES, hash), pick(&NOUNS, hash >> 16));
        let tail = pick(&NOUNS, hash >> 40);
        match (hash >> 32) % 6 {
            0 => format!("{title} (Live)"),
            1 => format!("The {title}"),
            2 => format!("{title} of the {tail}"),
            3 => format!("{title} {tail}"),
            _ => title,
        }
    }

    fn song_duration(index: usize) -> u64 {
        90_000 + (mix(index as u64 ^ 0xd0) % 330) * 1000
    }

    fn song_genre(index: usize) -> usize {
        (mix((index / SONGS_PER_ALBUM) as u64 ^ 0x6e) % GENRES.len() as u64) as usize
    }

    pub fn song_at(&self, index: usize) -> Song {
        let album = index / SONGS_PER_ALBUM;
        let artist = Self::album_artist(album);
        let hash = mix(index as u64 ^ 0x5eed);
        let genre = Self::song_genre(index);
        Song {
            title: Self::song_title(index),
            artists: vec![Self::artist_ref(artist)],
            album_artists: vec![Self::artist_ref(artist)],
            album: Some(Self::album_name(album)),
            album_id: Some(format!("album-{album}")),
            track_number: Some((index % SONGS_PER_ALBUM) as u32 + 1),
            disc_number: Some(1),
            year: Some(Self::album_year(album)),
            genres: vec![GENRES[genre].to_owned()],
            bitrate_kbps: Some(320),
            play_count: (hash % 60) as u32,
            user_rating: Some((hash >> 8) as u8 % 6).filter(|r| *r > 0),
            favorite: hash.is_multiple_of(9),
            cover_art_id: Some(format!("al-{album}")),
            container: Some("flac".into()),
            created_at: Some(format!("2024-01-01T00:00:{:02}Z", index % 60)),
            ..blank_song(
                &format!("song-{index}"),
                &self.config.id,
                Self::song_duration(index),
            )
        }
    }

    fn album_at(&self, album: usize) -> Album {
        let first = album * SONGS_PER_ALBUM;
        let last = (first + SONGS_PER_ALBUM).min(self.song_count);
        let artist = Self::album_artist(album);
        Album {
            id: format!("album-{album}"),
            server_id: self.config.id.clone(),
            name: Self::album_name(album),
            album_artists: vec![Self::artist_ref(artist)],
            artists: vec![Self::artist_ref(artist)],
            year: Some(Self::album_year(album)),
            genres: vec![GENRES[Self::song_genre(first)].to_owned()],
            song_count: (last - first) as u32,
            duration_ms: (first..last).map(Self::song_duration).sum(),
            cover_art_id: Some(format!("al-{album}")),
            play_count: 0,
            user_rating: None,
            favorite: false,
            created_at: None,
            last_played_at: None,
        }
    }

    fn artist_at(&self, artist: usize) -> Artist {
        Artist {
            id: format!("artist-{artist}"),
            server_id: self.config.id.clone(),
            name: Self::artist_name(artist),
            album_count: None,
            song_count: None,
            cover_art_id: None,
            favorite: false,
            user_rating: None,
            biography: None,
        }
    }

    fn matches(&self, index: usize, query: &SongListQuery, search: Option<&str>) -> bool {
        let album = index / SONGS_PER_ALBUM;
        let artist = Self::album_artist(album);
        if let Some(search) = search {
            let hit = Self::song_title(index).to_lowercase().contains(search)
                || Self::album_name(album).to_lowercase().contains(search)
                || Self::artist_name(artist).to_lowercase().contains(search);
            if !hit {
                return false;
            }
        }
        let year = Self::album_year(album);
        let filters = [
            query
                .favorite
                .is_none_or(|favorite| favorite == mix(index as u64 ^ 0x5eed).is_multiple_of(9)),
            query.album_ids.is_empty() || query.album_ids.contains(&format!("album-{album}")),
            query.artist_ids.is_empty() || query.artist_ids.contains(&format!("artist-{artist}")),
            query.genre_ids.is_empty()
                || query
                    .genre_ids
                    .contains(&format!("genre-{}", Self::song_genre(index))),
            query.year_from.is_none_or(|from| year >= from),
            query.year_to.is_none_or(|to| year <= to),
        ];
        filters.into_iter().all(|ok| ok)
    }

    fn sorted(&self, query: &SongListQuery) -> Arc<Vec<usize>> {
        if let Ok(cache) = self.order_cache.lock()
            && let Some((cached, order)) = cache.as_ref()
            && cached == query
        {
            return Arc::clone(order);
        }
        let search = query
            .search
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_lowercase);
        let mut order: Vec<usize> = (0..self.song_count)
            .filter(|&i| self.matches(i, query, search.as_deref()))
            .collect();
        let text_key = |i: usize| -> String {
            let album = i / SONGS_PER_ALBUM;
            match query.sort {
                SongSort::Album => format!(
                    "{}\u{0}{album:05}\u{0}{:02}",
                    Self::album_name(album),
                    i % SONGS_PER_ALBUM
                ),
                SongSort::Artist | SongSort::AlbumArtist => format!(
                    "{}\u{0}{}",
                    Self::artist_name(Self::album_artist(album)),
                    Self::album_name(album)
                ),
                _ => Self::song_title(i),
            }
            .to_lowercase()
        };
        match query.sort {
            SongSort::Name | SongSort::Album | SongSort::Artist | SongSort::AlbumArtist => {
                let mut keyed: Vec<(String, usize)> =
                    order.iter().map(|&i| (text_key(i), i)).collect();
                keyed.sort();
                order = keyed.into_iter().map(|(_, i)| i).collect();
            }
            sort => {
                let key = |i: usize| -> i64 {
                    let song = self.song_at(i);
                    match sort {
                        SongSort::Duration => song.duration_ms as i64,
                        SongSort::Year => i64::from(song.year.unwrap_or_default()),
                        SongSort::PlayCount => i64::from(song.play_count),
                        SongSort::Rating => i64::from(song.user_rating.unwrap_or_default()),
                        SongSort::Favorited => i64::from(song.favorite),
                        SongSort::RecentlyAdded => -(i as i64),
                        SongSort::RecentlyPlayed => (mix(i as u64 ^ 0x91a) % 10_000) as i64,
                        _ => mix(i as u64 ^ 0x4a4d) as i64,
                    }
                };
                order.sort_by_key(|&i| (key(i), i));
            }
        }
        if query.order == SortOrder::Desc {
            order.reverse();
        }
        let order = Arc::new(order);
        if let Ok(mut cache) = self.order_cache.lock() {
            *cache = Some((query.clone(), Arc::clone(&order)));
        }
        order
    }

    fn playlists(&self) -> std::sync::MutexGuard<'_, DemoPlaylists> {
        self.playlists
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn song_indexes(&self, song_ids: &[String]) -> Vec<usize> {
        song_ids
            .iter()
            .filter_map(|id| parse_index(id, "song-"))
            .filter(|&i| i < self.song_count)
            .collect()
    }

    fn to_playlist(&self, playlist: &DemoPlaylist) -> Playlist {
        Playlist {
            id: playlist.id.clone(),
            server_id: self.config.id.clone(),
            name: playlist.name.clone(),
            comment: playlist.comment.clone(),
            owner: Some(self.config.username.clone()),
            public: false,
            song_count: playlist.songs.len() as u32,
            duration_ms: playlist.songs.iter().map(|&i| Self::song_duration(i)).sum(),
            cover_art_id: Some(format!("pl-{}", playlist.id)),
            created_at: None,
            updated_at: None,
        }
    }

    fn page<T>(items: impl ExactSizeIterator<Item = T>, paging: Paging) -> Page<T> {
        let total = items.len();
        Page {
            items: items.skip(paging.start_index).take(paging.limit).collect(),
            start_index: paging.start_index,
            total_count: Some(total),
        }
    }
}

#[async_trait]
impl MusicServer for FakeServer {
    fn config(&self) -> &ServerConfig {
        &self.config
    }

    async fn ping(&self) -> Result<()> {
        Ok(())
    }

    async fn song_list(&self, query: &SongListQuery, paging: Paging) -> Result<Page<Song>> {
        let order = self.sorted(query);
        Ok(Self::page(order.iter().map(|&i| self.song_at(i)), paging))
    }

    async fn song_count(&self, query: &SongListQuery) -> Result<usize> {
        Ok(self.sorted(query).len())
    }

    async fn song(&self, id: &str) -> Result<Song> {
        parse_index(id, "song-")
            .filter(|&i| i < self.song_count)
            .map(|i| self.song_at(i))
            .ok_or(ApiError::NotFound)
    }

    async fn random_songs(&self, count: usize, _genre: Option<&str>) -> Result<Vec<Song>> {
        let seed = mix(count as u64);
        Ok((0..count.min(self.song_count))
            .map(|i| self.song_at((mix(seed ^ i as u64) % self.song_count as u64) as usize))
            .collect())
    }

    async fn similar_songs(&self, _song_id: &str, _count: usize) -> Result<Vec<Song>> {
        Ok(Vec::new())
    }

    async fn album_list(&self, _query: &AlbumListQuery, paging: Paging) -> Result<Page<Album>> {
        Ok(Self::page(
            (0..self.album_count()).map(|a| self.album_at(a)),
            paging,
        ))
    }

    async fn album_count(&self, _query: &AlbumListQuery) -> Result<usize> {
        Ok(self.album_count())
    }

    async fn album(&self, id: &str) -> Result<AlbumDetail> {
        let album = parse_index(id, "album-")
            .filter(|&a| a < self.album_count())
            .ok_or(ApiError::NotFound)?;
        let first = album * SONGS_PER_ALBUM;
        let last = (first + SONGS_PER_ALBUM).min(self.song_count);
        Ok(AlbumDetail {
            album: self.album_at(album),
            songs: (first..last).map(|i| self.song_at(i)).collect(),
        })
    }

    async fn artist_list(&self, _query: &ArtistListQuery, paging: Paging) -> Result<Page<Artist>> {
        Ok(Self::page(
            (0..ARTIST_COUNT).map(|a| self.artist_at(a)),
            paging,
        ))
    }

    async fn artist(&self, id: &str) -> Result<ArtistDetail> {
        let artist = parse_index(id, "artist-")
            .filter(|&a| a < ARTIST_COUNT)
            .ok_or(ApiError::NotFound)?;
        Ok(ArtistDetail {
            artist: self.artist_at(artist),
            albums: (0..self.album_count())
                .filter(|&a| Self::album_artist(a) == artist)
                .map(|a| self.album_at(a))
                .collect(),
            top_songs: Vec::new(),
        })
    }

    async fn genre_list(&self) -> Result<Vec<Genre>> {
        Ok(GENRES
            .iter()
            .enumerate()
            .map(|(i, name)| Genre {
                id: format!("genre-{i}"),
                name: (*name).to_owned(),
                song_count: None,
                album_count: None,
            })
            .collect())
    }

    async fn music_folders(&self) -> Result<Vec<MusicFolder>> {
        Ok(vec![MusicFolder {
            id: "folder-0".into(),
            name: "Music".into(),
        }])
    }

    async fn playlist_list(&self, paging: Paging) -> Result<Page<Playlist>> {
        let playlists = self.playlists();
        Ok(Self::page(
            playlists.items.iter().map(|p| self.to_playlist(p)),
            paging,
        ))
    }

    async fn playlist_songs(&self, id: &str) -> Result<Vec<Song>> {
        let mut playlists = self.playlists();
        let playlist = playlists.get_mut(id)?;
        Ok(playlist.songs.iter().map(|&i| self.song_at(i)).collect())
    }

    async fn create_playlist(&self, name: &str, song_ids: &[String]) -> Result<Playlist> {
        let songs = self.song_indexes(song_ids);
        let mut playlists = self.playlists();
        let playlist = playlists.push(name, None, songs);
        Ok(self.to_playlist(playlist))
    }

    async fn add_to_playlist(&self, playlist_id: &str, song_ids: &[String]) -> Result<()> {
        let songs = self.song_indexes(song_ids);
        let mut playlists = self.playlists();
        playlists.get_mut(playlist_id)?.songs.extend(songs);
        Ok(())
    }

    async fn remove_from_playlist(&self, playlist_id: &str, indexes: &[usize]) -> Result<()> {
        let mut playlists = self.playlists();
        let playlist = playlists.get_mut(playlist_id)?;
        let mut position = 0;
        playlist.songs.retain(|_| {
            position += 1;
            !indexes.contains(&(position - 1))
        });
        Ok(())
    }

    async fn delete_playlist(&self, id: &str) -> Result<()> {
        let mut playlists = self.playlists();
        let before = playlists.items.len();
        playlists.items.retain(|p| p.id != id);
        if playlists.items.len() == before {
            return Err(ApiError::NotFound);
        }
        Ok(())
    }

    async fn search(&self, query: &str, limit: usize) -> Result<SearchResults> {
        let query = SongListQuery {
            search: Some(query.to_owned()),
            ..SongListQuery::default()
        };
        let paging = Paging {
            start_index: 0,
            limit,
        };
        Ok(SearchResults {
            songs: self.song_list(&query, paging).await?.items,
            albums: Vec::new(),
            artists: Vec::new(),
        })
    }

    async fn set_favorite(&self, _kind: ItemKind, _ids: &[String], _favorite: bool) -> Result<()> {
        Ok(())
    }

    async fn set_rating(&self, _kind: ItemKind, _id: &str, _rating: u8) -> Result<()> {
        Ok(())
    }

    async fn scrobble(
        &self,
        song_id: &str,
        submission: bool,
        timestamp_ms: Option<i64>,
    ) -> Result<()> {
        if let Ok(mut calls) = self.scrobbles.lock() {
            calls.push(ScrobbleCall {
                song_id: song_id.to_owned(),
                submission,
                timestamp_ms,
            });
        }
        Ok(())
    }

    async fn save_play_queue(
        &self,
        _song_ids: &[String],
        _current_song_id: Option<&str>,
        _position_ms: u64,
    ) -> Result<()> {
        Ok(())
    }

    async fn play_queue(&self) -> Result<Option<ServerPlayQueue>> {
        Ok(None)
    }

    async fn lyrics(&self, _song: &Song) -> Result<Option<String>> {
        Ok(None)
    }

    fn stream_url(&self, song: &Song, _options: &StreamOptions) -> String {
        format!("demo://stream/{}", song.id)
    }

    fn cover_art_url(&self, cover_art_id: &str, size: Option<u32>) -> String {
        format!("demo://cover/{cover_art_id}?size={}", size.unwrap_or(0))
    }
}

pub fn demo_credential() -> Credential {
    Credential::Navidrome {
        subsonic: "u=demo".into(),
        nd_token: None,
    }
}

/// Deterministic artwork for demo covers.
pub fn render_cover(cover_art_id: &str, size: u32) -> Vec<u8> {
    let seed = cover_art_id
        .bytes()
        .fold(0u64, |acc, b| mix(acc ^ u64::from(b)));
    let base = [
        (seed & 0xff) as u32,
        ((seed >> 8) & 0xff) as u32,
        ((seed >> 16) & 0xff) as u32,
    ];
    let size = size.max(1);
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let t = (x + y) * 255 / (2 * size);
            let ring = ((x as i64 - size as i64 / 2).pow(2) + (y as i64 - size as i64 / 2).pow(2))
                .unsigned_abs()
                * 4
                / u64::from(size * size / 4).max(1);
            let shade = if ring.is_multiple_of(2) { 40 } else { 0 };
            rgba.extend_from_slice(&[
                ((base[0] * (255 - t) + base[2] * t) / 255 + shade).min(255) as u8,
                ((base[1] * (255 - t) + base[0] * t) / 255 + shade).min(255) as u8,
                ((base[2] * (255 - t) + base[1] * t) / 255 + shade).min(255) as u8,
                255,
            ]);
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pages_cover_the_whole_sorted_list() {
        let server = FakeServer::new(1000);
        let query = SongListQuery {
            sort: SongSort::Name,
            ..SongListQuery::default()
        };
        assert_eq!(server.song_count(&query).await.ok(), Some(1000));
        let first = server
            .song_list(
                &query,
                Paging {
                    start_index: 0,
                    limit: 10,
                },
            )
            .await
            .map(|p| p.items)
            .unwrap_or_default();
        let second = server
            .song_list(
                &query,
                Paging {
                    start_index: 10,
                    limit: 10,
                },
            )
            .await
            .map(|p| p.items)
            .unwrap_or_default();
        assert!(
            first.last().map(|s| s.title.to_lowercase())
                <= second.first().map(|s| s.title.to_lowercase())
        );
    }

    #[tokio::test]
    async fn search_filters_songs() {
        let server = FakeServer::new(1000);
        let query = SongListQuery {
            search: Some("river".into()),
            ..SongListQuery::default()
        };
        let count = server.song_count(&query).await.unwrap_or_default();
        assert!(count > 0 && count < 1000);
    }
}
