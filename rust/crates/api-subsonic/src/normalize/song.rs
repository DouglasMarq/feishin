use feishin_core::model::{ReplayGain, Song};

use super::artists::{artist_refs, genre_names};
use super::numbers::{
    capped_rating, non_empty_text, non_negative_size, non_negative_u32, nonzero_year, positive_u32,
    seconds_to_milliseconds, single_precision,
};
use crate::types::{self, SubsonicSong};

fn replay_gain(gain: types::ReplayGain) -> Option<ReplayGain> {
    let has_any_value = [
        gain.album_gain,
        gain.track_gain,
        gain.album_peak,
        gain.track_peak,
    ]
    .iter()
    .any(|value| value.is_some_and(|number| number != 0.0));
    has_any_value.then(|| ReplayGain {
        track_gain_db: single_precision(gain.track_gain),
        track_peak: single_precision(gain.track_peak),
        album_gain_db: single_precision(gain.album_gain),
        album_peak: single_precision(gain.album_peak),
    })
}

fn audio_container(content_type: Option<&str>) -> Option<String> {
    content_type
        .and_then(|mime_type| mime_type.strip_prefix("audio/"))
        .map(str::to_owned)
}

pub fn song(item: SubsonicSong, server_id: &str) -> Song {
    Song {
        id: item.id,
        server_id: server_id.to_owned(),
        title: item.title,
        artists: artist_refs(
            item.artists,
            item.artist_id.as_deref(),
            item.artist.as_deref(),
            item.contributors.as_deref(),
        ),
        album_artists: artist_refs(
            item.album_artists,
            item.artist_id.as_deref(),
            item.artist.as_deref(),
            None,
        ),
        album: non_empty_text(item.album),
        album_id: non_empty_text(item.album_id),
        duration_ms: seconds_to_milliseconds(item.duration),
        track_number: Some(positive_u32(item.track).unwrap_or(1)),
        disc_number: Some(positive_u32(item.disc_number).unwrap_or(1)),
        year: nonzero_year(item.year),
        genres: genre_names(item.genres, item.genre),
        bitrate_kbps: positive_u32(item.bit_rate),
        play_count: non_negative_u32(item.play_count).unwrap_or(0),
        user_rating: capped_rating(item.user_rating),
        favorite: item.starred.is_starred(),
        cover_art_id: non_empty_text(item.cover_art),
        replay_gain: item.replay_gain.and_then(replay_gain),
        path: non_empty_text(item.path),
        size_bytes: non_negative_size(item.size),
        container: audio_container(item.content_type.as_deref()),
        created_at: item.created,
        last_played_at: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use feishin_core::model::ArtistRef;
    use serde_json::{Value, json};

    fn parse_song(value: Value) -> SubsonicSong {
        serde_json::from_value(value).unwrap_or_default()
    }

    #[test]
    fn song_maps_fields() {
        let item = parse_song(json!({
            "id": 12, "title": "T", "album": "A", "albumId": "al-1", "artist": "X", "artistId": "ar-1",
            "artists": [{"id": "ar-1", "name": "X"}],
            "albumArtists": [{"id": "ar-2", "name": "Y"}],
            "contributors": [
                {"role": "remixer", "artist": {"id": "ar-3", "name": "R"}},
                {"role": "remixer", "subRole": "dub", "artist": {"id": "ar-4", "name": "D"}}
            ],
            "duration": 215, "track": 0, "discNumber": "2", "year": 2001, "bitRate": 320,
            "playCount": 7, "userRating": 4, "size": 1000, "contentType": "audio/flac",
            "coverArt": "mf-12", "created": "2024-01-01T00:00:00Z", "genres": [{"name": "Rock"}],
            "path": "a/b.flac", "starred": "2024-01-02T00:00:00Z",
            "replayGain": {"trackGain": -6.5, "trackPeak": 0.9}
        }));
        let song = song(item, "srv");
        assert_eq!(song.id, "12");
        assert_eq!(song.artists.len(), 2);
        assert_eq!(song.artists[1].id, "ar-3");
        assert_eq!(song.album_artists[0].name, "Y");
        assert_eq!(song.duration_ms, 215_000);
        assert_eq!(song.track_number, Some(1));
        assert_eq!(song.disc_number, Some(2));
        assert_eq!(song.year, Some(2001));
        assert_eq!(song.container.as_deref(), Some("flac"));
        assert_eq!(song.user_rating, Some(4));
        assert!(song.favorite);
        assert_eq!(song.genres, vec!["Rock".to_owned()]);
        let gain = song.replay_gain.unwrap_or_default();
        assert_eq!(gain.track_gain_db, Some(-6.5));
        assert_eq!(gain.album_gain_db, None);
    }

    #[test]
    fn song_without_artist_arrays_falls_back_to_artist_field() {
        let song = song(
            parse_song(
                json!({"id": "1", "title": "T", "artist": "X", "artistId": "9", "genre": "Pop"}),
            ),
            "srv",
        );
        assert_eq!(
            song.artists,
            vec![ArtistRef {
                id: "9".into(),
                name: "X".into()
            }]
        );
        assert_eq!(song.album_artists, song.artists);
        assert_eq!(song.genres, vec!["Pop".to_owned()]);
        assert!(!song.favorite);
        assert_eq!(song.replay_gain, None);
    }
}
