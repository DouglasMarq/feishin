//! Navidrome native API (`/api/*`) responses and their normalization (port of `navidrome-normalize.ts`).

use std::collections::HashMap;

use feishin_core::model::{Album, Artist, ArtistRef, Genre, Playlist, Song};
use serde::Deserialize;

use crate::util::{
    non_empty, opt_f32, opt_f64, opt_i32, opt_string, opt_u32, opt_u64, partial_iso_year,
    play_date, rating, replay_gain, seconds_to_ms, string, truthy,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NdAuth {
    pub token: String,
    pub subsonic_salt: String,
    pub subsonic_token: String,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct NdRef {
    #[serde(default, deserialize_with = "string")]
    pub id: String,
    #[serde(default, deserialize_with = "string")]
    pub name: String,
}

type Participants = HashMap<String, Vec<NdRef>>;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct NdSong {
    #[serde(deserialize_with = "string")]
    pub id: String,
    #[serde(deserialize_with = "opt_string")]
    pub media_file_id: Option<String>,
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
    #[serde(deserialize_with = "opt_string")]
    pub album_artist: Option<String>,
    #[serde(deserialize_with = "opt_string")]
    pub album_artist_id: Option<String>,
    pub participants: Option<Participants>,
    #[serde(deserialize_with = "opt_f64")]
    pub duration: Option<f64>,
    #[serde(deserialize_with = "opt_u32")]
    pub track_number: Option<u32>,
    #[serde(deserialize_with = "opt_u32")]
    pub disc_number: Option<u32>,
    #[serde(deserialize_with = "opt_i32")]
    pub year: Option<i32>,
    #[serde(deserialize_with = "opt_string")]
    pub date: Option<String>,
    pub genres: Option<Vec<NdRef>>,
    #[serde(deserialize_with = "opt_u32")]
    pub bit_rate: Option<u32>,
    #[serde(deserialize_with = "opt_u32")]
    pub play_count: Option<u32>,
    #[serde(deserialize_with = "opt_u32")]
    pub rating: Option<u32>,
    #[serde(deserialize_with = "truthy")]
    pub starred: bool,
    #[serde(deserialize_with = "opt_f32")]
    pub rg_album_gain: Option<f32>,
    #[serde(deserialize_with = "opt_f32")]
    pub rg_album_peak: Option<f32>,
    #[serde(deserialize_with = "opt_f32")]
    pub rg_track_gain: Option<f32>,
    #[serde(deserialize_with = "opt_f32")]
    pub rg_track_peak: Option<f32>,
    #[serde(deserialize_with = "opt_string")]
    pub path: Option<String>,
    #[serde(deserialize_with = "opt_string")]
    pub library_path: Option<String>,
    #[serde(deserialize_with = "opt_u64")]
    pub size: Option<u64>,
    #[serde(deserialize_with = "opt_string")]
    pub suffix: Option<String>,
    #[serde(deserialize_with = "opt_string")]
    pub created_at: Option<String>,
    #[serde(deserialize_with = "opt_string")]
    pub play_date: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct NdAlbum {
    #[serde(deserialize_with = "string")]
    pub id: String,
    #[serde(deserialize_with = "string")]
    pub name: String,
    #[serde(deserialize_with = "opt_string")]
    pub album_artist: Option<String>,
    #[serde(deserialize_with = "opt_string")]
    pub album_artist_id: Option<String>,
    #[serde(deserialize_with = "opt_string")]
    pub artist: Option<String>,
    #[serde(deserialize_with = "opt_string")]
    pub artist_id: Option<String>,
    pub participants: Option<Participants>,
    #[serde(deserialize_with = "opt_string")]
    pub cover_art_id: Option<String>,
    #[serde(deserialize_with = "opt_u32")]
    pub song_count: Option<u32>,
    #[serde(deserialize_with = "opt_f64")]
    pub duration: Option<f64>,
    pub genres: Option<Vec<NdRef>>,
    #[serde(deserialize_with = "opt_i32")]
    pub min_year: Option<i32>,
    #[serde(deserialize_with = "opt_string")]
    pub date: Option<String>,
    #[serde(deserialize_with = "opt_string")]
    pub release_date: Option<String>,
    #[serde(deserialize_with = "opt_u32")]
    pub play_count: Option<u32>,
    #[serde(deserialize_with = "opt_u32")]
    pub rating: Option<u32>,
    #[serde(deserialize_with = "truthy")]
    pub starred: bool,
    #[serde(deserialize_with = "opt_string")]
    pub created_at: Option<String>,
    #[serde(deserialize_with = "opt_string")]
    pub play_date: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct NdStats {
    #[serde(deserialize_with = "opt_u32")]
    pub album_count: Option<u32>,
    #[serde(deserialize_with = "opt_u32")]
    pub song_count: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct NdArtist {
    #[serde(deserialize_with = "string")]
    pub id: String,
    #[serde(deserialize_with = "string")]
    pub name: String,
    #[serde(deserialize_with = "opt_u32")]
    pub album_count: Option<u32>,
    #[serde(deserialize_with = "opt_u32")]
    pub song_count: Option<u32>,
    pub stats: Option<HashMap<String, NdStats>>,
    #[serde(deserialize_with = "truthy")]
    pub starred: bool,
    #[serde(deserialize_with = "opt_u32")]
    pub rating: Option<u32>,
    #[serde(deserialize_with = "opt_string")]
    pub biography: Option<String>,
    #[serde(deserialize_with = "opt_string")]
    pub uploaded_image: Option<String>,
    #[serde(deserialize_with = "opt_string")]
    pub updated_at: Option<String>,
    #[serde(deserialize_with = "opt_string")]
    pub external_info_updated_at: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct NdPlaylist {
    #[serde(deserialize_with = "string")]
    pub id: String,
    #[serde(deserialize_with = "string")]
    pub name: String,
    #[serde(deserialize_with = "opt_string")]
    pub comment: Option<String>,
    #[serde(deserialize_with = "opt_string")]
    pub owner_name: Option<String>,
    #[serde(deserialize_with = "truthy")]
    pub public: bool,
    #[serde(deserialize_with = "opt_u32")]
    pub song_count: Option<u32>,
    #[serde(deserialize_with = "opt_f64")]
    pub duration: Option<f64>,
    #[serde(deserialize_with = "opt_string")]
    pub created_at: Option<String>,
    #[serde(deserialize_with = "opt_string")]
    pub updated_at: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct NdTag {
    #[serde(deserialize_with = "string")]
    pub id: String,
    #[serde(deserialize_with = "string")]
    pub tag_value: String,
    #[serde(deserialize_with = "opt_u32")]
    pub album_count: Option<u32>,
    #[serde(deserialize_with = "opt_u32")]
    pub song_count: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct NdQueue {
    #[serde(deserialize_with = "opt_u32")]
    pub current: Option<u32>,
    pub items: Option<Vec<NdSong>>,
    #[serde(deserialize_with = "opt_u64")]
    pub position: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct NdId {
    #[serde(deserialize_with = "string")]
    pub id: String,
}

fn refs(list: &[NdRef]) -> Vec<ArtistRef> {
    list.iter()
        .map(|r| ArtistRef {
            id: r.id.clone(),
            name: r.name.clone(),
        })
        .collect()
}

struct ArtistFields<'a> {
    participants: Option<&'a Participants>,
    album_artist_id: Option<&'a str>,
    album_artist: Option<&'a str>,
    artist_id: Option<&'a str>,
    artist: Option<&'a str>,
}

/// Returns `(album_artists, artists)`, preferring the `participants` roles.
fn related_artists(
    fields: ArtistFields<'_>,
    include_remixers: bool,
) -> (Vec<ArtistRef>, Vec<ArtistRef>) {
    let mut album_artists = None;
    let mut artists = Vec::new();
    let mut remixers = Vec::new();

    if let Some(participants) = fields.participants {
        if let Some(list) = participants.get("albumartist") {
            album_artists = Some(refs(list));
        }
        if let Some(list) = participants.get("artist") {
            artists = refs(list);
        }
        if include_remixers && let Some(list) = participants.get("remixer") {
            remixers = refs(list);
        }
    }

    let album_artists = album_artists.unwrap_or_else(|| {
        vec![ArtistRef {
            id: fields.album_artist_id.unwrap_or_default().to_owned(),
            name: fields.album_artist.unwrap_or_default().to_owned(),
        }]
    });

    if artists.is_empty()
        && let (Some(id), Some(name)) = (fields.artist_id, fields.artist)
        && !id.is_empty()
        && !name.is_empty()
    {
        artists.push(ArtistRef {
            id: id.to_owned(),
            name: name.to_owned(),
        });
    }

    for remixer in remixers {
        if !artists.iter().any(|a| a.id == remixer.id) {
            artists.push(remixer);
        }
    }

    (album_artists, artists)
}

fn genre_names(genres: Option<&Vec<NdRef>>) -> Vec<String> {
    genres
        .map(|g| g.iter().map(|g| g.name.clone()).collect())
        .unwrap_or_default()
}

pub(crate) fn song(item: NdSong, server_id: &str) -> Song {
    let (album_artists, artists) = related_artists(
        ArtistFields {
            participants: item.participants.as_ref(),
            album_artist_id: item.album_artist_id.as_deref(),
            album_artist: item.album_artist.as_deref(),
            artist_id: item.artist_id.as_deref(),
            artist: item.artist.as_deref(),
        },
        true,
    );

    let year = item
        .year
        .filter(|y| *y > 0)
        .or_else(|| partial_iso_year(item.date.as_deref()).filter(|y| *y > 0));

    let path = non_empty(item.path).map(|p| match item.library_path.as_deref() {
        Some(lib) if !lib.is_empty() => format!("{lib}/{p}"),
        _ => p,
    });

    let id = item.media_file_id.unwrap_or(item.id);

    Song {
        cover_art_id: Some(id.clone()),
        id,
        server_id: server_id.to_owned(),
        title: item.title,
        artists,
        album_artists,
        album: item.album,
        album_id: non_empty(item.album_id),
        duration_ms: seconds_to_ms(item.duration),
        track_number: item.track_number,
        disc_number: item.disc_number,
        year,
        genres: genre_names(item.genres.as_ref()),
        bitrate_kbps: item.bit_rate,
        play_count: item.play_count.unwrap_or(0),
        user_rating: rating(item.rating),
        favorite: item.starred,
        replay_gain: replay_gain(
            item.rg_album_gain,
            item.rg_album_peak,
            item.rg_track_gain,
            item.rg_track_peak,
        ),
        path,
        size_bytes: item.size,
        container: item.suffix,
        created_at: item.created_at,
        last_played_at: play_date(item.play_date),
    }
}

pub(crate) fn album(item: NdAlbum, server_id: &str) -> Album {
    let (album_artists, artists) = related_artists(
        ArtistFields {
            participants: item.participants.as_ref(),
            album_artist_id: item.album_artist_id.as_deref(),
            album_artist: item.album_artist.as_deref(),
            artist_id: item.artist_id.as_deref(),
            artist: item.artist.as_deref(),
        },
        false,
    );

    let year = partial_iso_year(item.release_date.as_deref())
        .or_else(|| partial_iso_year(item.date.as_deref()))
        .or(item.min_year.filter(|y| *y != 0));

    Album {
        cover_art_id: Some(non_empty(item.cover_art_id).unwrap_or_else(|| item.id.clone())),
        id: item.id,
        server_id: server_id.to_owned(),
        name: item.name,
        album_artists,
        artists,
        year,
        genres: genre_names(item.genres.as_ref()),
        song_count: item.song_count.unwrap_or(0),
        duration_ms: seconds_to_ms(item.duration),
        play_count: item.play_count.unwrap_or(0),
        user_rating: rating(item.rating),
        favorite: item.starred,
        created_at: item.created_at,
        last_played_at: play_date(item.play_date),
    }
}

pub(crate) fn artist(item: NdArtist, server_id: &str) -> Artist {
    let (album_count, song_count) = match &item.stats {
        Some(stats) => {
            let pick = |f: fn(&NdStats) -> Option<u32>| {
                let a = stats.get("albumartist").and_then(f).unwrap_or(0);
                let b = stats.get("artist").and_then(f).unwrap_or(0);
                a.max(b)
            };
            (pick(|s| s.album_count), pick(|s| s.song_count))
        }
        None => (item.album_count.unwrap_or(0), item.song_count.unwrap_or(0)),
    };

    let cover_art_id = match non_empty(item.uploaded_image) {
        Some(_) => format!(
            "{}&_={}",
            item.id,
            item.updated_at
                .or(item.external_info_updated_at)
                .unwrap_or_default()
        ),
        None => item.id.clone(),
    };

    Artist {
        id: item.id,
        server_id: server_id.to_owned(),
        name: item.name,
        album_count: Some(album_count),
        song_count: Some(song_count),
        cover_art_id: Some(cover_art_id),
        favorite: item.starred,
        user_rating: rating(item.rating),
        biography: non_empty(item.biography),
    }
}

pub(crate) fn playlist(item: NdPlaylist, server_id: &str) -> Playlist {
    let cover_art_id = format!("{}&_={}", item.id, item.updated_at.as_deref().unwrap_or(""));
    Playlist {
        id: item.id,
        server_id: server_id.to_owned(),
        name: item.name,
        comment: non_empty(item.comment),
        owner: non_empty(item.owner_name),
        public: item.public,
        song_count: item.song_count.unwrap_or(0),
        duration_ms: seconds_to_ms(item.duration),
        cover_art_id: Some(cover_art_id),
        created_at: item.created_at,
        updated_at: item.updated_at,
    }
}

pub(crate) fn genre(item: NdTag) -> Genre {
    Genre {
        id: item.id,
        name: item.tag_value,
        song_count: item.song_count,
        album_count: item.album_count,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn song_prefers_participants_and_appends_remixers() {
        let raw: NdSong = serde_json::from_value(json!({
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
        let s = song(raw, "srv");
        assert_eq!(s.artists.len(), 2);
        assert_eq!(s.artists[1].id, "rx");
        assert_eq!(s.album_artists[0].id, "aa1");
        assert_eq!(s.duration_ms, 201_500);
        assert_eq!(s.year, Some(1999));
        assert_eq!(s.user_rating, None);
        assert!(s.favorite);
        assert_eq!(s.path.as_deref(), Some("/music/a/b.flac"));
        assert_eq!(s.last_played_at, None);
        assert!(s.genres.is_empty());
    }

    #[test]
    fn album_without_participants_falls_back_to_fields() {
        let raw: NdAlbum = serde_json::from_value(json!({
            "id": "al1",
            "name": "Album",
            "albumArtist": "AA",
            "albumArtistId": "aa1",
            "minYear": 2004,
            "releaseDate": "",
            "songCount": 3
        }))
        .expect("fixture");
        let a = album(raw, "srv");
        assert_eq!(a.album_artists[0].name, "AA");
        assert!(a.artists.is_empty());
        assert_eq!(a.year, Some(2004));
        assert_eq!(a.cover_art_id.as_deref(), Some("al1"));
    }

    #[test]
    fn artist_counts_from_stats_and_cache_bust() {
        let raw: NdArtist = serde_json::from_value(json!({
            "id": "ar1",
            "name": "Artist",
            "albumCount": 1,
            "songCount": 1,
            "stats": {
                "albumartist": {"albumCount": 2, "songCount": 20},
                "artist": {"albumCount": 3, "songCount": 10}
            },
            "uploadedImage": "x.jpg",
            "updatedAt": "2024-01-01"
        }))
        .expect("fixture");
        let a = artist(raw, "srv");
        assert_eq!(a.album_count, Some(3));
        assert_eq!(a.song_count, Some(20));
        assert_eq!(a.cover_art_id.as_deref(), Some("ar1&_=2024-01-01"));
    }
}
