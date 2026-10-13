use serde::Deserialize;

use crate::subsonic::album::SubsonicAlbum;
use crate::subsonic::artist::SubsonicArtist;
use crate::subsonic::song::SubsonicSong;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct SearchResult {
    pub song: Vec<SubsonicSong>,
    pub album: Vec<SubsonicAlbum>,
    pub artist: Vec<SubsonicArtist>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Search3 {
    pub search_result3: Option<SearchResult>,
}
