use feishin_core::model::Song;
use serde::Deserialize;

use crate::conversions::{ReplayGainValues, non_empty, rating, replay_gain, seconds_to_ms};
use crate::lenient::{
    optional_f32, optional_f64, optional_i32, optional_string, optional_u32, optional_u64, string,
    truthy,
};
use crate::subsonic::references::{
    SubsonicGenreName, SubsonicReference, artist_refs_or_single, genre_names,
};

const AUDIO_CONTENT_TYPE_PREFIX: &str = "audio/";
const FIRST_POSITION: u32 = 1;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct SubsonicReplayGain {
    #[serde(deserialize_with = "optional_f32")]
    pub track_gain: Option<f32>,
    #[serde(deserialize_with = "optional_f32")]
    pub track_peak: Option<f32>,
    #[serde(deserialize_with = "optional_f32")]
    pub album_gain: Option<f32>,
    #[serde(deserialize_with = "optional_f32")]
    pub album_peak: Option<f32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct SubsonicSong {
    #[serde(deserialize_with = "string")]
    pub id: String,
    #[serde(deserialize_with = "string")]
    pub title: String,
    #[serde(deserialize_with = "optional_string")]
    pub album: Option<String>,
    #[serde(deserialize_with = "optional_string")]
    pub album_id: Option<String>,
    #[serde(deserialize_with = "optional_string")]
    pub artist: Option<String>,
    #[serde(deserialize_with = "optional_string")]
    pub artist_id: Option<String>,
    pub artists: Option<Vec<SubsonicReference>>,
    pub album_artists: Option<Vec<SubsonicReference>>,
    #[serde(deserialize_with = "optional_f64")]
    pub duration: Option<f64>,
    #[serde(deserialize_with = "optional_u32")]
    pub track: Option<u32>,
    #[serde(deserialize_with = "optional_u32")]
    pub disc_number: Option<u32>,
    #[serde(deserialize_with = "optional_i32")]
    pub year: Option<i32>,
    #[serde(deserialize_with = "optional_string")]
    pub genre: Option<String>,
    pub genres: Option<Vec<SubsonicGenreName>>,
    #[serde(deserialize_with = "optional_u32")]
    pub bit_rate: Option<u32>,
    #[serde(deserialize_with = "optional_u32")]
    pub play_count: Option<u32>,
    #[serde(deserialize_with = "optional_u32")]
    pub user_rating: Option<u32>,
    #[serde(deserialize_with = "truthy")]
    pub starred: bool,
    #[serde(deserialize_with = "optional_string")]
    pub cover_art: Option<String>,
    pub replay_gain: Option<SubsonicReplayGain>,
    #[serde(deserialize_with = "optional_string")]
    pub path: Option<String>,
    #[serde(deserialize_with = "optional_u64")]
    pub size: Option<u64>,
    #[serde(deserialize_with = "optional_string")]
    pub content_type: Option<String>,
    #[serde(deserialize_with = "optional_string")]
    pub created: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct SongList {
    pub song: Vec<SubsonicSong>,
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
    pub song: Option<SubsonicSong>,
}

pub(crate) fn normalize_song(song: SubsonicSong, server_id: &str) -> Song {
    let album_artists = artist_refs_or_single(
        song.album_artists.as_ref(),
        song.artist_id.as_deref(),
        song.artist.as_deref(),
    );
    let artists = artist_refs_or_single(
        song.artists.as_ref(),
        song.artist_id.as_deref(),
        song.artist.as_deref(),
    );
    let container = audio_container(song.content_type.as_deref());
    let replay_gain = song_replay_gain(song.replay_gain.unwrap_or_default());

    Song {
        id: song.id,
        server_id: server_id.to_owned(),
        title: song.title,
        artists,
        album_artists,
        album: non_empty(song.album),
        album_id: non_empty(song.album_id),
        duration_ms: seconds_to_ms(song.duration),
        track_number: Some(position_or_first(song.track)),
        disc_number: Some(position_or_first(song.disc_number)),
        year: song.year.filter(|year| *year != 0),
        genres: genre_names(song.genres, song.genre),
        bitrate_kbps: song.bit_rate,
        play_count: song.play_count.unwrap_or(0),
        user_rating: rating(song.user_rating),
        favorite: song.starred,
        cover_art_id: non_empty(song.cover_art),
        replay_gain,
        path: non_empty(song.path),
        size_bytes: song.size,
        container,
        created_at: song.created,
        last_played_at: None,
    }
}

fn audio_container(content_type: Option<&str>) -> Option<String> {
    content_type
        .and_then(|content_type| content_type.strip_prefix(AUDIO_CONTENT_TYPE_PREFIX))
        .map(str::to_owned)
}

fn position_or_first(position: Option<u32>) -> u32 {
    position
        .filter(|position| *position > 0)
        .unwrap_or(FIRST_POSITION)
}

fn song_replay_gain(gain: SubsonicReplayGain) -> Option<feishin_core::model::ReplayGain> {
    replay_gain(ReplayGainValues {
        album_gain: gain.album_gain,
        album_peak: gain.album_peak,
        track_gain: gain.track_gain,
        track_peak: gain.track_peak,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn song_from_subsonic() {
        let raw: SubsonicSong = serde_json::from_value(json!({
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
        let song = normalize_song(raw, "srv");
        assert_eq!(song.id, "12");
        assert_eq!(song.artists[0].id, "a1");
        assert_eq!(song.album_artists[0].name, "A");
        assert_eq!(song.container.as_deref(), Some("flac"));
        assert_eq!(song.track_number, Some(1));
        assert!(song.favorite);
        assert_eq!(song.user_rating, Some(4));
        assert_eq!(song.genres, vec!["Rock".to_owned()]);
        assert_eq!(
            song.replay_gain.and_then(|gain| gain.track_gain_db),
            Some(-3.5)
        );
    }
}
