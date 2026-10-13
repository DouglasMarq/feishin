use feishin_core::model::Genre;
use serde::Deserialize;

use crate::lenient::{optional_u32, string};

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct NativeTag {
    #[serde(deserialize_with = "string")]
    pub id: String,
    #[serde(deserialize_with = "string")]
    pub tag_value: String,
    #[serde(deserialize_with = "optional_u32")]
    pub album_count: Option<u32>,
    #[serde(deserialize_with = "optional_u32")]
    pub song_count: Option<u32>,
}

pub(crate) fn normalize_genre(tag: NativeTag) -> Genre {
    Genre {
        id: tag.id,
        name: tag.tag_value,
        song_count: tag.song_count,
        album_count: tag.album_count,
    }
}
