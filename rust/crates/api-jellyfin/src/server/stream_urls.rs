use feishin_api::{StreamOptions, TranscodeFormat};

use super::JellyfinServer;
use crate::BITS_PER_KILOBIT;
use crate::transport::endpoint;

const DEFAULT_TRANSCODE_FORMAT: &str = "mp3";
const IMAGE_QUALITY: &str = "96";
const TRANSCODING_PROTOCOL: &str = "http";

impl JellyfinServer {
    pub(super) fn song_stream_url(&self, song_id: &str, options: &StreamOptions) -> String {
        match (&options.format, options.max_bitrate_kbps) {
            (None, None) => self.direct_download_url(song_id),
            _ => self.transcoding_url(song_id, options),
        }
    }

    pub(super) fn primary_image_url(&self, cover_art_id: &str, edge_pixels: Option<u32>) -> String {
        let mut url = endpoint(
            &self.base_url,
            &["Items", cover_art_id, "Images", "Primary"],
        );
        let mut query = url.query_pairs_mut();
        query.append_pair("quality", IMAGE_QUALITY);
        if let Some(edge) = edge_pixels.map(|edge| edge.to_string()) {
            query
                .append_pair("fillHeight", &edge)
                .append_pair("fillWidth", &edge);
        }
        query.finish().to_string()
    }

    fn direct_download_url(&self, song_id: &str) -> String {
        let mut url = endpoint(&self.base_url, &["Items", song_id, "Download"]);
        url.query_pairs_mut()
            .append_pair("apiKey", &self.access_token)
            .append_pair("playSessionId", "")
            .finish()
            .to_string()
    }

    fn transcoding_url(&self, song_id: &str, options: &StreamOptions) -> String {
        let format = options
            .format
            .as_ref()
            .map_or(DEFAULT_TRANSCODE_FORMAT, TranscodeFormat::as_str);
        let session = format!("feishin-{song_id}-{format}-0-0");
        let mut url = endpoint(&self.base_url, &["audio", song_id, "universal"]);
        let mut query = url.query_pairs_mut();
        query
            .append_pair("userId", &self.user_id)
            .append_pair("deviceId", "")
            .append_pair("audioCodec", format)
            .append_pair("apiKey", &self.access_token)
            .append_pair("playSessionId", &session)
            .append_pair("container", format)
            .append_pair("transcodingProtocol", TRANSCODING_PROTOCOL)
            .append_pair("transcodingContainer", format);
        if let Some(kbps) = options.max_bitrate_kbps {
            let bits_per_second = u64::from(kbps) * BITS_PER_KILOBIT;
            query.append_pair("maxStreamingBitrate", &bits_per_second.to_string());
        }
        query.finish().to_string()
    }
}
