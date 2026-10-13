use feishin_core::model::Song;
use serde::Deserialize;

use crate::conversions::{
    ReplayGainValues, non_empty, partial_iso_year, play_date, rating, replay_gain, seconds_to_ms,
};
use crate::lenient::{
    optional_f32, optional_f64, optional_i32, optional_string, optional_u32, optional_u64, string,
    truthy,
};
use crate::native::related_artists::{
    ArtistFields, NativeReference, Participants, Remixers, genre_names, related_artists,
};

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct NativeSong {
    #[serde(deserialize_with = "string")]
    pub id: String,
    #[serde(deserialize_with = "optional_string")]
    pub media_file_id: Option<String>,
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
    #[serde(deserialize_with = "optional_string")]
    pub album_artist: Option<String>,
    #[serde(deserialize_with = "optional_string")]
    pub album_artist_id: Option<String>,
    pub participants: Option<Participants>,
    #[serde(deserialize_with = "optional_f64")]
    pub duration: Option<f64>,
    #[serde(deserialize_with = "optional_u32")]
    pub track_number: Option<u32>,
    #[serde(deserialize_with = "optional_u32")]
    pub disc_number: Option<u32>,
    #[serde(deserialize_with = "optional_i32")]
    pub year: Option<i32>,
    #[serde(deserialize_with = "optional_string")]
    pub date: Option<String>,
    pub genres: Option<Vec<NativeReference>>,
    #[serde(deserialize_with = "optional_u32")]
    pub bit_rate: Option<u32>,
    #[serde(deserialize_with = "optional_u32")]
    pub play_count: Option<u32>,
    #[serde(deserialize_with = "optional_u32")]
    pub rating: Option<u32>,
    #[serde(deserialize_with = "truthy")]
    pub starred: bool,
    #[serde(rename = "rgAlbumGain", deserialize_with = "optional_f32")]
    pub album_replay_gain: Option<f32>,
    #[serde(rename = "rgAlbumPeak", deserialize_with = "optional_f32")]
    pub album_replay_peak: Option<f32>,
    #[serde(rename = "rgTrackGain", deserialize_with = "optional_f32")]
    pub track_replay_gain: Option<f32>,
    #[serde(rename = "rgTrackPeak", deserialize_with = "optional_f32")]
    pub track_replay_peak: Option<f32>,
    #[serde(deserialize_with = "optional_string")]
    pub path: Option<String>,
    #[serde(deserialize_with = "optional_string")]
    pub library_path: Option<String>,
    #[serde(deserialize_with = "optional_u64")]
    pub size: Option<u64>,
    #[serde(deserialize_with = "optional_string")]
    pub suffix: Option<String>,
    #[serde(deserialize_with = "optional_string")]
    pub created_at: Option<String>,
    #[serde(deserialize_with = "optional_string")]
    pub play_date: Option<String>,
}

pub(crate) fn normalize_song(song: NativeSong, server_id: &str) -> Song {
    let related = related_artists(&artist_fields(&song), Remixers::Include);
    let year = song_year(&song);
    let path = full_path(song.path, song.library_path.as_deref());
    let replay_gain = replay_gain(ReplayGainValues {
        album_gain: song.album_replay_gain,
        album_peak: song.album_replay_peak,
        track_gain: song.track_replay_gain,
        track_peak: song.track_replay_peak,
    });
    let id = song.media_file_id.unwrap_or(song.id);

    Song {
        cover_art_id: Some(id.clone()),
        id,
        server_id: server_id.to_owned(),
        title: song.title,
        artists: related.artists,
        album_artists: related.album_artists,
        album: song.album,
        album_id: non_empty(song.album_id),
        duration_ms: seconds_to_ms(song.duration),
        track_number: song.track_number,
        disc_number: song.disc_number,
        year,
        genres: genre_names(song.genres.as_ref()),
        bitrate_kbps: song.bit_rate,
        play_count: song.play_count.unwrap_or(0),
        user_rating: rating(song.rating),
        favorite: song.starred,
        replay_gain,
        path,
        size_bytes: song.size,
        container: song.suffix,
        created_at: song.created_at,
        last_played_at: play_date(song.play_date),
    }
}

fn artist_fields(song: &NativeSong) -> ArtistFields<'_> {
    ArtistFields {
        participants: song.participants.as_ref(),
        album_artist_id: song.album_artist_id.as_deref(),
        album_artist: song.album_artist.as_deref(),
        artist_id: song.artist_id.as_deref(),
        artist: song.artist.as_deref(),
    }
}

fn song_year(song: &NativeSong) -> Option<i32> {
    song.year
        .filter(|year| *year > 0)
        .or_else(|| partial_iso_year(song.date.as_deref()).filter(|year| *year > 0))
}

fn full_path(path: Option<String>, library_path: Option<&str>) -> Option<String> {
    non_empty(path).map(|relative| match library_path {
        Some(library) if !library.is_empty() => format!("{library}/{relative}"),
        _ => relative,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn song_prefers_participants_and_appends_remixers() {
        let raw: NativeSong = serde_json::from_value(json!({
            "id": "s1",
            "title": "Song",
            "album": "Album",
            "albumId": "al1",
            "artist": "Fallback",
            "artistId": "ar0",
            "albumArtist": "AA",
            "albumArtistId": "aa1",
            "participants": {
                "artist": [{"id": "ar1", "name": "One"}],
                "albumartist": [{"id": "aa1", "name": "AA"}],
                "remixer": [{"id": "ar1", "name": "One"}, {"id": "rx", "name": "Remixer"}]
            },
            "duration": 201.5,
            "year": 0,
            "date": "1999-05",
            "rating": 0,
            "starred": true,
            "path": "a/b.flac",
            "libraryPath": "/music",
            "playDate": "0001-01-01T00:00:00Z",
            "genres": null
        }))
        .expect("fixture");
        let song = normalize_song(raw, "srv");
        assert_eq!(song.artists.len(), 2);
        assert_eq!(song.artists[1].id, "rx");
        assert_eq!(song.album_artists[0].id, "aa1");
        assert_eq!(song.duration_ms, 201_500);
        assert_eq!(song.year, Some(1999));
        assert_eq!(song.user_rating, None);
        assert!(song.favorite);
        assert_eq!(song.path.as_deref(), Some("/music/a/b.flac"));
        assert_eq!(song.last_played_at, None);
        assert!(song.genres.is_empty());
    }
}
