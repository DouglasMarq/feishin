use feishin_core::model::Artist;
use serde::Deserialize;

use crate::conversions::{non_empty, rating};
use crate::lenient::{optional_string, optional_u32, string, truthy};
use crate::subsonic::album::SubsonicAlbum;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct SubsonicArtist {
    #[serde(deserialize_with = "string")]
    pub id: String,
    #[serde(deserialize_with = "string")]
    pub name: String,
    #[serde(deserialize_with = "optional_u32")]
    pub album_count: Option<u32>,
    #[serde(deserialize_with = "optional_string")]
    pub cover_art: Option<String>,
    #[serde(deserialize_with = "truthy")]
    pub starred: bool,
    #[serde(deserialize_with = "optional_u32")]
    pub user_rating: Option<u32>,
    pub album: Option<Vec<SubsonicAlbum>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct ArtistResponse {
    pub artist: Option<SubsonicArtist>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct ArtistIndex {
    pub artist: Vec<SubsonicArtist>,
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
    #[serde(deserialize_with = "optional_string")]
    pub biography: Option<String>,
}

pub(crate) fn normalize_artist(artist: SubsonicArtist, server_id: &str) -> Artist {
    Artist {
        id: artist.id,
        server_id: server_id.to_owned(),
        name: artist.name,
        album_count: Some(artist.album_count.unwrap_or(0)),
        song_count: None,
        cover_art_id: non_empty(artist.cover_art),
        favorite: artist.starred,
        user_rating: rating(artist.user_rating),
        biography: None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn artist_album_count_as_string() {
        let raw: SubsonicArtist =
            serde_json::from_value(json!({"id": "1", "name": "N", "albumCount": "7"}))
                .expect("fixture");
        assert_eq!(normalize_artist(raw, "srv").album_count, Some(7));
    }
}
