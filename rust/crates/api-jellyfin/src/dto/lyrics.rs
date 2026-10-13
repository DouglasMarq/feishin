use serde::Deserialize;

use super::null_as_default;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Lyrics {
    #[serde(deserialize_with = "null_as_default")]
    pub lyrics: Vec<LyricLine>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct LyricLine {
    pub start: Option<u64>,
    #[serde(deserialize_with = "null_as_default")]
    pub text: String,
}
