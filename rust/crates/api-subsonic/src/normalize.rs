//! Port of `src/shared/api/subsonic/subsonic-normalize.ts`.

use feishin_core::model::{
    Album, Artist, ArtistRef, Genre, MusicFolder, Playlist, ReplayGain, Song,
};
use serde_json::Value;

use crate::types::{
    Contributor, GenreItem, SimpleArtist, SsAlbum, SsArtist, SsGenre, SsMusicFolder, SsPlaylist,
    SsSong,
};

pub fn truthy(value: &Option<Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

fn u32_of(value: Option<f64>) -> Option<u32> {
    value
        .filter(|v| v.is_finite() && *v >= 0.0)
        .map(|v| v.min(f64::from(u32::MAX)) as u32)
}

fn nonzero_u32(value: Option<f64>) -> Option<u32> {
    u32_of(value).filter(|v| *v > 0)
}

fn ms(seconds: Option<f64>) -> u64 {
    seconds
        .filter(|v| v.is_finite() && *v > 0.0)
        .map_or(0, |v| (v * 1000.0).round() as u64)
}

fn year(value: Option<f64>) -> Option<i32> {
    value
        .filter(|v| v.is_finite() && *v != 0.0)
        .map(|v| v as i32)
}

fn rating(value: Option<f64>) -> Option<u8> {
    nonzero_u32(value).map(|v| v.min(5) as u8)
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|s| !s.is_empty())
}

fn gain(value: Option<f64>) -> Option<f32> {
    value.map(|v| v as f32)
}

fn simple(artists: Vec<SimpleArtist>) -> impl Iterator<Item = ArtistRef> {
    artists.into_iter().map(|a| ArtistRef {
        id: a.id,
        name: a.name,
    })
}

fn artist_list(
    artists: Option<Vec<SimpleArtist>>,
    artist_id: Option<&str>,
    artist_name: Option<&str>,
    contributors: Option<&[Contributor]>,
) -> Vec<ArtistRef> {
    if artists.is_none() && contributors.is_none() {
        let id = artist_id.unwrap_or_default();
        let name = artist_name.unwrap_or_default();
        if id.is_empty() && name.is_empty() {
            return Vec::new();
        }
        return vec![ArtistRef {
            id: id.to_owned(),
            name: name.to_owned(),
        }];
    }

    let mut result: Vec<ArtistRef> = simple(artists.unwrap_or_default()).collect();
    for contributor in contributors.unwrap_or_default() {
        if contributor.role == "remixer"
            && contributor.sub_role.is_none()
            && !result.iter().any(|a| a.id == contributor.artist.id)
        {
            result.push(ArtistRef {
                id: contributor.artist.id.clone(),
                name: contributor.artist.name.clone(),
            });
        }
    }
    result
}

fn genres(genres: Option<Vec<GenreItem>>, genre: Option<String>) -> Vec<String> {
    match genres {
        Some(list) => list.into_iter().map(|g| g.name).collect(),
        None => non_empty(genre).into_iter().collect(),
    }
}

pub fn song(item: SsSong, server_id: &str) -> Song {
    let artists = artist_list(
        item.artists,
        item.artist_id.as_deref(),
        item.artist.as_deref(),
        item.contributors.as_deref(),
    );
    let album_artists = artist_list(
        item.album_artists,
        item.artist_id.as_deref(),
        item.artist.as_deref(),
        None,
    );
    let replay_gain = item.replay_gain.and_then(|g| {
        let any = [g.album_gain, g.track_gain, g.album_peak, g.track_peak]
            .iter()
            .any(|v| v.is_some_and(|v| v != 0.0));
        any.then(|| ReplayGain {
            track_gain_db: gain(g.track_gain),
            track_peak: gain(g.track_peak),
            album_gain_db: gain(g.album_gain),
            album_peak: gain(g.album_peak),
        })
    });
    let container = item
        .content_type
        .as_deref()
        .and_then(|c| c.strip_prefix("audio/"))
        .map(str::to_owned);

    Song {
        id: item.id,
        server_id: server_id.to_owned(),
        title: item.title,
        artists,
        album_artists,
        album: non_empty(item.album),
        album_id: non_empty(item.album_id),
        duration_ms: ms(item.duration),
        track_number: Some(nonzero_u32(item.track).unwrap_or(1)),
        disc_number: Some(nonzero_u32(item.disc_number).unwrap_or(1)),
        year: year(item.year),
        genres: genres(item.genres, item.genre),
        bitrate_kbps: nonzero_u32(item.bit_rate),
        play_count: u32_of(item.play_count).unwrap_or(0),
        user_rating: rating(item.user_rating),
        favorite: truthy(&item.starred),
        cover_art_id: non_empty(item.cover_art),
        replay_gain,
        path: non_empty(item.path),
        size_bytes: item
            .size
            .filter(|v| v.is_finite() && *v >= 0.0)
            .map(|v| v as u64),
        container,
        created_at: item.created,
        last_played_at: None,
    }
}

fn release_year(item: &SsAlbum) -> Option<i32> {
    match &item.release_date {
        Some(rd) if rd.year.is_some() && rd.month.is_some() && rd.day.is_some() => {
            year(rd.year).or_else(|| year(item.year))
        }
        _ => year(item.year),
    }
}

pub fn album(item: SsAlbum, server_id: &str) -> (Album, Vec<Song>) {
    let year = release_year(&item);
    let album_artists = artist_list(
        item.artists,
        item.artist_id.as_deref(),
        item.artist.as_deref(),
        None,
    );
    let songs = item
        .song
        .unwrap_or_default()
        .into_iter()
        .map(|s| song(s, server_id))
        .collect();
    let album = Album {
        id: item.id,
        server_id: server_id.to_owned(),
        name: item.name.or(item.title).or(item.album).unwrap_or_default(),
        album_artists,
        artists: Vec::new(),
        year,
        genres: genres(item.genres, item.genre),
        song_count: u32_of(item.song_count).unwrap_or(0),
        duration_ms: ms(item.duration),
        cover_art_id: non_empty(item.cover_art),
        play_count: u32_of(item.play_count).unwrap_or(0),
        user_rating: rating(item.user_rating),
        favorite: truthy(&item.starred),
        created_at: item.created,
        last_played_at: None,
    };
    (album, songs)
}

pub fn album_only(item: SsAlbum, server_id: &str) -> Album {
    album(item, server_id).0
}

pub fn artist(item: SsArtist, server_id: &str) -> Artist {
    Artist {
        id: item.id,
        server_id: server_id.to_owned(),
        name: item.name,
        album_count: Some(u32_of(item.album_count).unwrap_or(0)),
        song_count: None,
        cover_art_id: non_empty(item.cover_art),
        favorite: truthy(&item.starred),
        user_rating: rating(item.user_rating),
        biography: None,
    }
}

pub fn playlist(item: SsPlaylist, server_id: &str) -> Playlist {
    Playlist {
        id: item.id,
        server_id: server_id.to_owned(),
        name: item.name,
        comment: non_empty(item.comment),
        owner: non_empty(item.owner),
        public: item.public.unwrap_or(false),
        song_count: u32_of(item.song_count).unwrap_or(0),
        duration_ms: ms(item.duration),
        cover_art_id: non_empty(item.cover_art),
        created_at: item.created,
        updated_at: item.changed,
    }
}

pub fn genre(item: SsGenre) -> Genre {
    Genre {
        id: item.value.clone(),
        name: item.value,
        song_count: u32_of(item.song_count),
        album_count: u32_of(item.album_count),
    }
}

pub fn music_folder(item: SsMusicFolder) -> MusicFolder {
    MusicFolder {
        id: item.id,
        name: item.name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse_song(value: Value) -> SsSong {
        serde_json::from_value(value).unwrap_or_default()
    }

    #[test]
    fn song_maps_fields() {
        let s = parse_song(json!({
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
        let song = song(s, "srv");
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

    #[test]
    fn album_prefers_full_release_date_year() {
        let item: SsAlbum = serde_json::from_value(json!({
            "id": "a1", "name": "N", "artist": "X", "artistId": "x", "year": 1999,
            "releaseDate": {"year": 2005, "month": 3, "day": 1}, "songCount": 3, "duration": 100,
            "song": [{"id": "s1", "title": "one"}]
        }))
        .unwrap_or_default();
        let (album, songs) = album(item, "srv");
        assert_eq!(album.year, Some(2005));
        assert_eq!(album.song_count, 3);
        assert_eq!(album.album_artists[0].id, "x");
        assert_eq!(songs.len(), 1);
    }
}
