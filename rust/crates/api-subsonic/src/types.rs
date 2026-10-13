//! Wire types for the Subsonic / OpenSubsonic JSON API (port of `src/shared/api/subsonic/subsonic-types.ts`).
//! Servers disagree on whether ids and counters are numbers or strings, so those fields are lenient.

use serde::{Deserialize, Deserializer};
use serde_json::Value;

fn id<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(opt_id(d)?.unwrap_or_default())
}

fn opt_id<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::String(s) => Some(s),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    })
}

fn num<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    })
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SimpleArtist {
    #[serde(deserialize_with = "id")]
    pub id: String,
    pub name: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Contributor {
    pub artist: SimpleArtist,
    pub role: String,
    pub sub_role: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct GenreItem {
    pub name: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ReplayGain {
    #[serde(deserialize_with = "num")]
    pub album_gain: Option<f64>,
    #[serde(deserialize_with = "num")]
    pub album_peak: Option<f64>,
    #[serde(deserialize_with = "num")]
    pub track_gain: Option<f64>,
    #[serde(deserialize_with = "num")]
    pub track_peak: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SsSong {
    #[serde(deserialize_with = "id")]
    pub id: String,
    pub title: String,
    pub album: Option<String>,
    #[serde(deserialize_with = "opt_id")]
    pub album_id: Option<String>,
    pub artist: Option<String>,
    #[serde(deserialize_with = "opt_id")]
    pub artist_id: Option<String>,
    pub artists: Option<Vec<SimpleArtist>>,
    pub album_artists: Option<Vec<SimpleArtist>>,
    pub contributors: Option<Vec<Contributor>>,
    #[serde(deserialize_with = "num")]
    pub duration: Option<f64>,
    #[serde(deserialize_with = "num")]
    pub track: Option<f64>,
    #[serde(deserialize_with = "num")]
    pub disc_number: Option<f64>,
    #[serde(deserialize_with = "num")]
    pub year: Option<f64>,
    #[serde(deserialize_with = "num")]
    pub bit_rate: Option<f64>,
    #[serde(deserialize_with = "num")]
    pub play_count: Option<f64>,
    #[serde(deserialize_with = "num")]
    pub user_rating: Option<f64>,
    #[serde(deserialize_with = "num")]
    pub size: Option<f64>,
    pub content_type: Option<String>,
    #[serde(deserialize_with = "opt_id")]
    pub cover_art: Option<String>,
    pub created: Option<String>,
    pub genre: Option<String>,
    pub genres: Option<Vec<GenreItem>>,
    pub path: Option<String>,
    pub replay_gain: Option<ReplayGain>,
    pub starred: Option<Value>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ReleaseDate {
    #[serde(deserialize_with = "num")]
    pub year: Option<f64>,
    #[serde(deserialize_with = "num")]
    pub month: Option<f64>,
    #[serde(deserialize_with = "num")]
    pub day: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SsAlbum {
    #[serde(deserialize_with = "id")]
    pub id: String,
    pub name: Option<String>,
    pub title: Option<String>,
    pub album: Option<String>,
    pub artist: Option<String>,
    #[serde(deserialize_with = "opt_id")]
    pub artist_id: Option<String>,
    pub artists: Option<Vec<SimpleArtist>>,
    #[serde(deserialize_with = "opt_id")]
    pub cover_art: Option<String>,
    pub created: Option<String>,
    #[serde(deserialize_with = "num")]
    pub duration: Option<f64>,
    pub genre: Option<String>,
    pub genres: Option<Vec<GenreItem>>,
    #[serde(deserialize_with = "num")]
    pub song_count: Option<f64>,
    #[serde(deserialize_with = "num")]
    pub play_count: Option<f64>,
    #[serde(deserialize_with = "num")]
    pub user_rating: Option<f64>,
    #[serde(deserialize_with = "num")]
    pub year: Option<f64>,
    pub release_date: Option<ReleaseDate>,
    pub starred: Option<Value>,
    pub song: Option<Vec<SsSong>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SsArtist {
    #[serde(deserialize_with = "id")]
    pub id: String,
    pub name: String,
    #[serde(deserialize_with = "num")]
    pub album_count: Option<f64>,
    #[serde(deserialize_with = "opt_id")]
    pub cover_art: Option<String>,
    #[serde(deserialize_with = "num")]
    pub user_rating: Option<f64>,
    pub starred: Option<Value>,
    pub album: Option<Vec<SsAlbum>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SsPlaylist {
    #[serde(deserialize_with = "id")]
    pub id: String,
    pub name: String,
    pub comment: Option<String>,
    pub owner: Option<String>,
    pub public: Option<bool>,
    #[serde(deserialize_with = "num")]
    pub song_count: Option<f64>,
    #[serde(deserialize_with = "num")]
    pub duration: Option<f64>,
    #[serde(deserialize_with = "opt_id")]
    pub cover_art: Option<String>,
    pub created: Option<String>,
    pub changed: Option<String>,
    pub entry: Option<Vec<SsSong>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SsGenre {
    pub value: String,
    #[serde(deserialize_with = "num")]
    pub song_count: Option<f64>,
    #[serde(deserialize_with = "num")]
    pub album_count: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct SsMusicFolder {
    #[serde(deserialize_with = "id")]
    pub id: String,
    pub name: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct SongContainer {
    pub song: Option<Vec<SsSong>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct SearchResult {
    pub song: Option<Vec<SsSong>>,
    pub album: Option<Vec<SsAlbum>>,
    pub artist: Option<Vec<SsArtist>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Search3 {
    pub search_result3: Option<SearchResult>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Starred2 {
    pub starred2: Option<SearchResult>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct AlbumContainer {
    pub album: Option<Vec<SsAlbum>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AlbumList2 {
    pub album_list2: Option<AlbumContainer>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SongsByGenre {
    pub songs_by_genre: Option<SongContainer>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RandomSongs {
    pub random_songs: Option<SongContainer>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SimilarSongs {
    pub similar_songs: Option<SongContainer>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TopSongs {
    pub top_songs: Option<SongContainer>,
}

#[derive(Debug, Deserialize)]
pub struct GetAlbum {
    pub album: SsAlbum,
}

#[derive(Debug, Deserialize)]
pub struct GetArtist {
    pub artist: SsArtist,
}

#[derive(Debug, Deserialize)]
pub struct GetSong {
    pub song: SsSong,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ArtistIndex {
    pub artist: Option<Vec<SsArtist>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ArtistIndexes {
    pub index: Option<Vec<ArtistIndex>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct GetArtists {
    pub artists: Option<ArtistIndexes>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ArtistInfo {
    pub biography: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ArtistInfo2 {
    pub artist_info2: Option<ArtistInfo>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct GenreContainer {
    pub genre: Option<Vec<SsGenre>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Genres {
    pub genres: Option<GenreContainer>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MusicFolderContainer {
    pub music_folder: Option<Vec<SsMusicFolder>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MusicFolders {
    pub music_folders: Option<MusicFolderContainer>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct PlaylistContainer {
    pub playlist: Option<Vec<SsPlaylist>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Playlists {
    pub playlists: Option<PlaylistContainer>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct PlaylistResponse {
    pub playlist: Option<SsPlaylist>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlayQueue {
    #[serde(deserialize_with = "opt_id")]
    pub current: Option<String>,
    #[serde(deserialize_with = "num")]
    pub current_index: Option<f64>,
    #[serde(deserialize_with = "num")]
    pub position: Option<f64>,
    pub entry: Option<Vec<SsSong>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GetPlayQueue {
    pub play_queue: Option<PlayQueue>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GetPlayQueueByIndex {
    pub play_queue_by_index: Option<PlayQueue>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Ping {
    pub open_subsonic: Option<bool>,
    pub server_version: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Extension {
    pub name: String,
    pub versions: Vec<u32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct OpenSubsonicExtensions {
    pub open_subsonic_extensions: Option<Vec<Extension>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct LyricLine {
    #[serde(deserialize_with = "num")]
    pub start: Option<f64>,
    pub value: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct StructuredLyric {
    pub kind: Option<String>,
    pub lang: Option<String>,
    pub line: Vec<LyricLine>,
    #[serde(deserialize_with = "num")]
    pub offset: Option<f64>,
    pub synced: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LyricsContainer {
    pub structured_lyrics: Option<Vec<StructuredLyric>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LyricsList {
    pub lyrics_list: Option<LyricsContainer>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Empty {}
