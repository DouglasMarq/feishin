use feishin_core::model::ArtistRef;
use serde::Deserialize;

use crate::conversions::non_empty;
use crate::lenient::string;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct SubsonicReference {
    #[serde(deserialize_with = "string")]
    pub id: String,
    #[serde(deserialize_with = "string")]
    pub name: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct SubsonicGenreName {
    #[serde(deserialize_with = "string")]
    pub name: String,
}

pub(crate) fn artist_refs_or_single(
    artists: Option<&Vec<SubsonicReference>>,
    artist_id: Option<&str>,
    artist_name: Option<&str>,
) -> Vec<ArtistRef> {
    match artists {
        Some(references) => references
            .iter()
            .map(|reference| ArtistRef {
                id: reference.id.clone(),
                name: reference.name.clone(),
            })
            .collect(),
        None => vec![ArtistRef {
            id: artist_id.unwrap_or_default().to_owned(),
            name: artist_name.unwrap_or_default().to_owned(),
        }],
    }
}

pub(crate) fn genre_names(
    genres: Option<Vec<SubsonicGenreName>>,
    single_genre: Option<String>,
) -> Vec<String> {
    match genres {
        Some(genres) => genres.into_iter().map(|genre| genre.name).collect(),
        None => non_empty(single_genre).into_iter().collect(),
    }
}
