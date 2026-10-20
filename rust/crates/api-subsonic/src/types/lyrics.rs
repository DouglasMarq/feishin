use serde::Deserialize;

use super::lenient::lenient_number;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct LyricLine {
    #[serde(deserialize_with = "lenient_number")]
    pub start: Option<f64>,
    pub value: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct StructuredLyric {
    pub kind: Option<String>,
    pub lang: Option<String>,
    pub line: Vec<LyricLine>,
    #[serde(deserialize_with = "lenient_number")]
    pub offset: Option<f64>,
    #[serde(rename = "synced")]
    pub is_synced: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LyricsContainer {
    pub structured_lyrics: Option<Vec<StructuredLyric>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LyricsList {
    pub lyrics_list: Option<LyricsContainer>,
}
