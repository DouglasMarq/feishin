use feishin_api::StreamOptions;
use feishin_core::model::Song;

use crate::NavidromeServer;
use crate::http::ResponseFormat;
use crate::query::QueryParams;

const CACHE_BUSTER_SEPARATOR: &str = "&_=";

impl NavidromeServer {
    pub(crate) fn build_stream_url(&self, song: &Song, options: &StreamOptions) -> String {
        let mut params: QueryParams = vec![("id", song.id.clone())];
        params.extend(options.format.clone().map(|format| ("format", format)));
        params.extend(
            options
                .max_bitrate_kbps
                .map(|bitrate| ("maxBitRate", bitrate.to_string())),
        );
        self.subsonic_url("stream", &params, ResponseFormat::ServerDefault)
    }

    pub(crate) fn build_cover_art_url(&self, cover_art_id: &str, size: Option<u32>) -> String {
        let mut params = cover_art_id_params(cover_art_id);
        params.extend(size.map(|size| ("size", size.to_string())));
        self.subsonic_url("getCoverArt", &params, ResponseFormat::ServerDefault)
    }
}

fn cover_art_id_params(cover_art_id: &str) -> QueryParams {
    match cover_art_id.split_once(CACHE_BUSTER_SEPARATOR) {
        Some((id, cache_buster)) => vec![("id", id.to_owned()), ("_", cache_buster.to_owned())],
        None => vec![("id", cover_art_id.to_owned())],
    }
}
