use std::collections::HashMap;

use feishin_core::model::Artist;
use serde::Deserialize;

use crate::conversions::{non_empty, rating};
use crate::lenient::{optional_string, optional_u32, string, truthy};

const ALBUM_ARTIST_ROLE: &str = "albumartist";
const ARTIST_ROLE: &str = "artist";

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct NativeArtistStats {
    #[serde(deserialize_with = "optional_u32")]
    pub album_count: Option<u32>,
    #[serde(deserialize_with = "optional_u32")]
    pub song_count: Option<u32>,
}

type StatsByRole = HashMap<String, NativeArtistStats>;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct NativeArtist {
    #[serde(deserialize_with = "string")]
    pub id: String,
    #[serde(deserialize_with = "string")]
    pub name: String,
    #[serde(deserialize_with = "optional_u32")]
    pub album_count: Option<u32>,
    #[serde(deserialize_with = "optional_u32")]
    pub song_count: Option<u32>,
    pub stats: Option<StatsByRole>,
    #[serde(deserialize_with = "truthy")]
    pub starred: bool,
    #[serde(deserialize_with = "optional_u32")]
    pub rating: Option<u32>,
    #[serde(deserialize_with = "optional_string")]
    pub biography: Option<String>,
    #[serde(deserialize_with = "optional_string")]
    pub uploaded_image: Option<String>,
    #[serde(deserialize_with = "optional_string")]
    pub updated_at: Option<String>,
    #[serde(deserialize_with = "optional_string")]
    pub external_info_updated_at: Option<String>,
}

struct ArtistCounts {
    albums: u32,
    songs: u32,
}

pub(crate) fn normalize_artist(artist: NativeArtist, server_id: &str) -> Artist {
    let counts = artist_counts(&artist);
    let cover_art_id = cover_art_id(&artist);

    Artist {
        id: artist.id,
        server_id: server_id.to_owned(),
        name: artist.name,
        album_count: Some(counts.albums),
        song_count: Some(counts.songs),
        cover_art_id: Some(cover_art_id),
        favorite: artist.starred,
        user_rating: rating(artist.rating),
        biography: non_empty(artist.biography),
    }
}

fn artist_counts(artist: &NativeArtist) -> ArtistCounts {
    match &artist.stats {
        Some(stats) => ArtistCounts {
            albums: highest_role_count(stats, |role_stats| role_stats.album_count),
            songs: highest_role_count(stats, |role_stats| role_stats.song_count),
        },
        None => ArtistCounts {
            albums: artist.album_count.unwrap_or(0),
            songs: artist.song_count.unwrap_or(0),
        },
    }
}

fn highest_role_count(stats: &StatsByRole, count_of: fn(&NativeArtistStats) -> Option<u32>) -> u32 {
    let as_album_artist = stats.get(ALBUM_ARTIST_ROLE).and_then(count_of).unwrap_or(0);
    let as_artist = stats.get(ARTIST_ROLE).and_then(count_of).unwrap_or(0);
    as_album_artist.max(as_artist)
}

fn cover_art_id(artist: &NativeArtist) -> String {
    let has_uploaded_image = artist
        .uploaded_image
        .as_deref()
        .is_some_and(|image| !image.is_empty());
    if !has_uploaded_image {
        return artist.id.clone();
    }
    let cache_buster = artist
        .updated_at
        .as_deref()
        .or(artist.external_info_updated_at.as_deref())
        .unwrap_or_default();
    format!("{}&_={cache_buster}", artist.id)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn artist_counts_from_stats_and_cache_bust() {
        let raw: NativeArtist = serde_json::from_value(json!({
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
        let artist = normalize_artist(raw, "srv");
        assert_eq!(artist.album_count, Some(3));
        assert_eq!(artist.song_count, Some(20));
        assert_eq!(artist.cover_art_id.as_deref(), Some("ar1&_=2024-01-01"));
    }
}
