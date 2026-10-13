//! Subsonic (`/rest/*.view`) responses and their normalization (port of `subsonic-normalize.ts`).

use feishin_core::model::{Album, Artist, ArtistRef, Genre, MusicFolder, Playlist, Song};
use serde::Deserialize;

use crate::util::{
    non_empty, opt_f32, opt_f64, opt_i32, opt_string, opt_u32, opt_u64, rating, replay_gain,
    seconds_to_ms, string, truthy,
};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct SsRef {
    #[serde(deserialize_with = "string")]
    pub id: String,
    #[serde(deserialize_with = "string")]
    pub name: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct SsGenreName {
    #[serde(deserialize_with = "string")]
    pub name: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct SsReplayGain {
    #[serde(deserialize_with = "opt_f32")]
    pub track_gain: Option<f32>,
    #[serde(deserialize_with = "opt_f32")]
    pub track_peak: Option<f32>,
    #[serde(deserialize_with = "opt_f32")]
    pub album_gain: Option<f32>,
    #[serde(deserialize_with = "opt_f32")]
    pub album_peak: Option<f32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct SsSong {
    #[serde(deserialize_with = "string")]
    pub id: String,
    #[serde(deserialize_with = "string")]
    pub title: String,
    #[serde(deserialize_with = "opt_string")]
    pub album: Option<String>,
    #[serde(deserialize_with = "opt_string")]
    pub album_id: Option<String>,
    #[serde(deserialize_with = "opt_string")]
    pub artist: Option<String>,
    #[serde(deserialize_with = "opt_string")]
    pub artist_id: Option<String>,
    pub artists: Option<Vec<SsRef>>,
    pub album_artists: Option<Vec<SsRef>>,
    #[serde(deserialize_with = "opt_f64")]
    pub duration: Option<f64>,
    #[serde(deserialize_with = "opt_u32")]
    pub track: Option<u32>,
    #[serde(deserialize_with = "opt_u32")]
    pub disc_number: Option<u32>,
    #[serde(deserialize_with = "opt_i32")]
    pub year: Option<i32>,
    #[serde(deserialize_with = "opt_string")]
    pub genre: Option<String>,
    pub genres: Option<Vec<SsGenreName>>,
    #[serde(deserialize_with = "opt_u32")]
    pub bit_rate: Option<u32>,
    #[serde(deserialize_with = "opt_u32")]
    pub play_count: Option<u32>,
    #[serde(deserialize_with = "opt_u32")]
    pub user_rating: Option<u32>,
    #[serde(deserialize_with = "truthy")]
    pub starred: bool,
    #[serde(deserialize_with = "opt_string")]
    pub cover_art: Option<String>,
    pub replay_gain: Option<SsReplayGain>,
    #[serde(deserialize_with = "opt_string")]
    pub path: Option<String>,
    #[serde(deserialize_with = "opt_u64")]
    pub size: Option<u64>,
    #[serde(deserialize_with = "opt_string")]
    pub content_type: Option<String>,
    #[serde(deserialize_with = "opt_string")]
    pub created: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct SsReleaseDate {
    #[serde(deserialize_with = "opt_i32")]
    pub year: Option<i32>,
    #[serde(deserialize_with = "opt_u32")]
    pub month: Option<u32>,
    #[serde(deserialize_with = "opt_u32")]
    pub day: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct SsAlbum {
    #[serde(deserialize_with = "string")]
    pub id: String,
    #[serde(deserialize_with = "string")]
    pub name: String,
    #[serde(deserialize_with = "opt_string")]
    pub artist: Option<String>,
    #[serde(deserialize_with = "opt_string")]
    pub artist_id: Option<String>,
    pub artists: Option<Vec<SsRef>>,
    #[serde(deserialize_with = "opt_string")]
    pub cover_art: Option<String>,
    #[serde(deserialize_with = "opt_u32")]
    pub song_count: Option<u32>,
    #[serde(deserialize_with = "opt_f64")]
    pub duration: Option<f64>,
    #[serde(deserialize_with = "opt_i32")]
    pub year: Option<i32>,
    pub release_date: Option<SsReleaseDate>,
    #[serde(deserialize_with = "opt_string")]
    pub genre: Option<String>,
    pub genres: Option<Vec<SsGenreName>>,
    #[serde(deserialize_with = "opt_u32")]
    pub play_count: Option<u32>,
    #[serde(deserialize_with = "opt_u32")]
    pub user_rating: Option<u32>,
    #[serde(deserialize_with = "truthy")]
    pub starred: bool,
    #[serde(deserialize_with = "opt_string")]
    pub created: Option<String>,
    #[serde(deserialize_with = "opt_string")]
    pub played: Option<String>,
    pub song: Option<Vec<SsSong>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct SsArtist {
    #[serde(deserialize_with = "string")]
    pub id: String,
    #[serde(deserialize_with = "string")]
    pub name: String,
    #[serde(deserialize_with = "opt_u32")]
    pub album_count: Option<u32>,
    #[serde(deserialize_with = "opt_string")]
    pub cover_art: Option<String>,
    #[serde(deserialize_with = "truthy")]
    pub starred: bool,
    #[serde(deserialize_with = "opt_u32")]
    pub user_rating: Option<u32>,
    pub album: Option<Vec<SsAlbum>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct SsPlaylist {
    #[serde(deserialize_with = "string")]
    pub id: String,
    #[serde(deserialize_with = "string")]
    pub name: String,
    #[serde(deserialize_with = "opt_string")]
    pub comment: Option<String>,
    #[serde(deserialize_with = "opt_string")]
    pub owner: Option<String>,
    #[serde(deserialize_with = "truthy")]
    pub public: bool,
    #[serde(deserialize_with = "opt_u32")]
    pub song_count: Option<u32>,
    #[serde(deserialize_with = "opt_f64")]
    pub duration: Option<f64>,
    #[serde(deserialize_with = "opt_string")]
    pub cover_art: Option<String>,
    #[serde(deserialize_with = "opt_string")]
    pub created: Option<String>,
    #[serde(deserialize_with = "opt_string")]
    pub changed: Option<String>,
    pub entry: Option<Vec<SsSong>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct SsGenre {
    #[serde(deserialize_with = "string")]
    pub value: String,
    #[serde(deserialize_with = "opt_u32")]
    pub song_count: Option<u32>,
    #[serde(deserialize_with = "opt_u32")]
    pub album_count: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct SsMusicFolder {
    #[serde(deserialize_with = "string")]
    pub id: String,
    #[serde(deserialize_with = "string")]
    pub name: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct SongList {
    pub song: Vec<SsSong>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct RandomSongs {
    pub random_songs: Option<SongList>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct SimilarSongs {
    pub similar_songs: Option<SongList>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct TopSongs {
    pub top_songs: Option<SongList>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct SongResponse {
    pub song: Option<SsSong>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct AlbumResponse {
    pub album: Option<SsAlbum>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct AlbumList {
    pub album: Vec<SsAlbum>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct AlbumList2 {
    pub album_list2: Option<AlbumList>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct ArtistResponse {
    pub artist: Option<SsArtist>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct ArtistIndex {
    pub artist: Vec<SsArtist>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct ArtistIndexes {
    pub index: Vec<ArtistIndex>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Artists {
    pub artists: Option<ArtistIndexes>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct ArtistInfo2 {
    pub artist_info2: Option<ArtistInfo>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct ArtistInfo {
    #[serde(deserialize_with = "opt_string")]
    pub biography: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct PlaylistResponse {
    pub playlist: Option<SsPlaylist>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct PlaylistList {
    pub playlist: Vec<SsPlaylist>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Playlists {
    pub playlists: Option<PlaylistList>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct GenreList {
    pub genre: Vec<SsGenre>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Genres {
    pub genres: Option<GenreList>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct FolderList {
    pub music_folder: Vec<SsMusicFolder>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct MusicFolders {
    pub music_folders: Option<FolderList>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct SearchResult {
    pub song: Vec<SsSong>,
    pub album: Vec<SsAlbum>,
    pub artist: Vec<SsArtist>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Search3 {
    pub search_result3: Option<SearchResult>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct PlayQueueBody {
    #[serde(deserialize_with = "opt_string")]
    pub current: Option<String>,
    #[serde(rename = "currentIndex", deserialize_with = "opt_u32")]
    pub current_index: Option<u32>,
    pub entry: Option<Vec<SsSong>>,
    #[serde(deserialize_with = "opt_u64")]
    pub position: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct PlayQueue {
    pub play_queue: Option<PlayQueueBody>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct PlayQueueByIndex {
    pub play_queue_by_index: Option<PlayQueueBody>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct LyricLine {
    #[serde(deserialize_with = "opt_u64")]
    pub start: Option<u64>,
    #[serde(deserialize_with = "string")]
    pub value: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct StructuredLyric {
    #[serde(deserialize_with = "opt_string")]
    pub kind: Option<String>,
    #[serde(deserialize_with = "truthy")]
    pub synced: bool,
    #[serde(deserialize_with = "opt_f64")]
    pub offset: Option<f64>,
    pub line: Vec<LyricLine>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct LyricsListBody {
    pub structured_lyrics: Vec<StructuredLyric>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct LyricsList {
    pub lyrics_list: Option<LyricsListBody>,
}

fn artist_list(
    artists: Option<&Vec<SsRef>>,
    artist_id: Option<&str>,
    artist_name: Option<&str>,
) -> Vec<ArtistRef> {
    match artists {
        Some(list) => list
            .iter()
            .map(|a| ArtistRef {
                id: a.id.clone(),
                name: a.name.clone(),
            })
            .collect(),
        None => vec![ArtistRef {
            id: artist_id.unwrap_or_default().to_owned(),
            name: artist_name.unwrap_or_default().to_owned(),
        }],
    }
}

fn genres(genres: Option<Vec<SsGenreName>>, genre: Option<String>) -> Vec<String> {
    match genres {
        Some(list) => list.into_iter().map(|g| g.name).collect(),
        None => non_empty(genre).into_iter().collect(),
    }
}

pub(crate) fn song(item: SsSong, server_id: &str) -> Song {
    let album_artists = artist_list(
        item.album_artists.as_ref(),
        item.artist_id.as_deref(),
        item.artist.as_deref(),
    );
    let artists = artist_list(
        item.artists.as_ref(),
        item.artist_id.as_deref(),
        item.artist.as_deref(),
    );
    let container = item
        .content_type
        .as_deref()
        .and_then(|ct| ct.strip_prefix("audio/"))
        .map(str::to_owned);
    let gain = item.replay_gain.unwrap_or_default();

    Song {
        id: item.id,
        server_id: server_id.to_owned(),
        title: item.title,
        artists,
        album_artists,
        album: non_empty(item.album),
        album_id: non_empty(item.album_id),
        duration_ms: seconds_to_ms(item.duration),
        track_number: Some(item.track.filter(|t| *t > 0).unwrap_or(1)),
        disc_number: Some(item.disc_number.filter(|d| *d > 0).unwrap_or(1)),
        year: item.year.filter(|y| *y != 0),
        genres: genres(item.genres, item.genre),
        bitrate_kbps: item.bit_rate,
        play_count: item.play_count.unwrap_or(0),
        user_rating: rating(item.user_rating),
        favorite: item.starred,
        cover_art_id: non_empty(item.cover_art),
        replay_gain: replay_gain(
            gain.album_gain,
            gain.album_peak,
            gain.track_gain,
            gain.track_peak,
        ),
        path: non_empty(item.path),
        size_bytes: item.size,
        container,
        created_at: item.created,
        last_played_at: None,
    }
}

pub(crate) fn album(item: SsAlbum, server_id: &str) -> Album {
    let year = item
        .release_date
        .as_ref()
        .and_then(|rd| match (rd.year, rd.month, rd.day) {
            (Some(y), Some(_), Some(_)) if y > 0 => Some(y),
            _ => None,
        })
        .or(item.year.filter(|y| *y != 0));

    Album {
        album_artists: artist_list(
            item.artists.as_ref(),
            item.artist_id.as_deref(),
            item.artist.as_deref(),
        ),
        artists: Vec::new(),
        id: item.id,
        server_id: server_id.to_owned(),
        name: item.name,
        year,
        genres: genres(item.genres, item.genre),
        song_count: item.song_count.unwrap_or(0),
        duration_ms: seconds_to_ms(item.duration),
        cover_art_id: non_empty(item.cover_art),
        play_count: item.play_count.unwrap_or(0),
        user_rating: rating(item.user_rating),
        favorite: item.starred,
        created_at: item.created,
        last_played_at: non_empty(item.played),
    }
}

pub(crate) fn artist(item: SsArtist, server_id: &str) -> Artist {
    Artist {
        id: item.id,
        server_id: server_id.to_owned(),
        name: item.name,
        album_count: Some(item.album_count.unwrap_or(0)),
        song_count: None,
        cover_art_id: non_empty(item.cover_art),
        favorite: item.starred,
        user_rating: rating(item.user_rating),
        biography: None,
    }
}

pub(crate) fn playlist(item: SsPlaylist, server_id: &str) -> Playlist {
    let cover_art_id = non_empty(item.cover_art).map(|c| match item.changed.as_deref() {
        Some(changed) if !changed.is_empty() => format!("{c}&_={changed}"),
        _ => c,
    });
    Playlist {
        id: item.id,
        server_id: server_id.to_owned(),
        name: item.name,
        comment: non_empty(item.comment),
        owner: non_empty(item.owner),
        public: item.public,
        song_count: item.song_count.unwrap_or(0),
        duration_ms: seconds_to_ms(item.duration),
        cover_art_id,
        created_at: item.created,
        updated_at: item.changed,
    }
}

pub(crate) fn genre(item: SsGenre) -> Genre {
    Genre {
        id: item.value.clone(),
        name: item.value,
        song_count: item.song_count,
        album_count: item.album_count,
    }
}

pub(crate) fn music_folder(item: SsMusicFolder) -> MusicFolder {
    MusicFolder {
        id: item.id,
        name: item.name,
    }
}

/// Picks the main lyric (synced first) and renders it as LRC, or plain text when unsynced.
pub(crate) fn lyrics_text(list: Vec<StructuredLyric>) -> Option<String> {
    let is_main = |l: &StructuredLyric| l.kind.as_deref().is_none_or(|k| k == "main");
    let mut list = list;
    let index = list
        .iter()
        .position(|l| is_main(l) && l.synced && !l.line.is_empty())
        .or_else(|| list.iter().position(|l| is_main(l) && !l.line.is_empty()))
        .or_else(|| list.iter().position(|l| !l.line.is_empty()))?;
    let lyric = list.swap_remove(index);

    if !lyric.synced {
        let text: Vec<String> = lyric.line.into_iter().map(|l| l.value).collect();
        return Some(text.join("\n"));
    }

    let mut out = Vec::with_capacity(lyric.line.len() + 1);
    if let Some(offset) = lyric.offset.filter(|o| *o != 0.0) {
        out.push(format!("[offset:{}]", offset.round() as i64));
    }
    for line in lyric.line {
        let ms = line.start.unwrap_or(0);
        let minutes = ms / 60_000;
        let seconds = (ms % 60_000) / 1000;
        let centis = (ms % 1000) / 10;
        out.push(format!(
            "[{minutes:02}:{seconds:02}.{centis:02}]{}",
            line.value
        ));
    }
    Some(out.join("\n"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn song_from_subsonic() {
        let raw: SsSong = serde_json::from_value(json!({
            "id": 12,
            "title": "T",
            "artist": "A",
            "artistId": "a1",
            "albumId": "al",
            "contentType": "audio/flac",
            "duration": 10,
            "starred": "2024-01-01T00:00:00Z",
            "userRating": 4,
            "genres": [{"name": "Rock"}],
            "replayGain": {"trackGain": -3.5}
        }))
        .expect("fixture");
        let s = song(raw, "srv");
        assert_eq!(s.id, "12");
        assert_eq!(s.artists[0].id, "a1");
        assert_eq!(s.album_artists[0].name, "A");
        assert_eq!(s.container.as_deref(), Some("flac"));
        assert_eq!(s.track_number, Some(1));
        assert!(s.favorite);
        assert_eq!(s.user_rating, Some(4));
        assert_eq!(s.genres, vec!["Rock".to_owned()]);
        assert_eq!(s.replay_gain.and_then(|r| r.track_gain_db), Some(-3.5));
    }

    #[test]
    fn artist_album_count_as_string() {
        let raw: SsArtist =
            serde_json::from_value(json!({"id": "1", "name": "N", "albumCount": "7"}))
                .expect("fixture");
        assert_eq!(artist(raw, "srv").album_count, Some(7));
    }

    #[test]
    fn synced_lyrics_render_as_lrc() {
        let list: LyricsList = serde_json::from_value(json!({
            "lyricsList": {"structuredLyrics": [
                {"lang": "eng", "synced": false, "line": [{"value": "plain"}]},
                {"lang": "eng", "synced": true, "offset": 100, "line": [
                    {"start": 0, "value": "first"},
                    {"start": 61_230, "value": "second"}
                ]}
            ]}
        }))
        .expect("fixture");
        let text = lyrics_text(list.lyrics_list.unwrap_or_default().structured_lyrics);
        assert_eq!(
            text.as_deref(),
            Some("[offset:100]\n[00:00.00]first\n[01:01.23]second")
        );
    }

    #[test]
    fn unsynced_lyrics_join_lines() {
        let lyric = StructuredLyric {
            kind: None,
            synced: false,
            offset: None,
            line: vec![
                LyricLine {
                    start: None,
                    value: "a".into(),
                },
                LyricLine {
                    start: None,
                    value: "b".into(),
                },
            ],
        };
        assert_eq!(lyrics_text(vec![lyric]).as_deref(), Some("a\nb"));
        assert_eq!(lyrics_text(Vec::new()), None);
    }
}
