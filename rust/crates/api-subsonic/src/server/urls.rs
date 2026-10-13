use feishin_api::StreamOptions;

use super::SubsonicServer;
use crate::client::{API_VERSION, CLIENT_NAME, encode_uri_component};

impl SubsonicServer {
    fn item_url(&self, endpoint: &str, id: &str) -> String {
        format!(
            "{}/rest/{endpoint}?id={}",
            self.client.base(),
            encode_uri_component(id)
        )
    }

    pub(super) fn build_stream_url(&self, song_id: &str, options: &StreamOptions) -> String {
        let mut url = format!(
            "{}&v={API_VERSION}&c={CLIENT_NAME}&{}",
            self.item_url("stream.view", song_id),
            self.client.auth()
        );
        if let Some(format) = options
            .format
            .as_deref()
            .filter(|format| !format.is_empty())
        {
            url.push_str("&format=");
            url.push_str(&encode_uri_component(format));
        }
        if let Some(bitrate) = options.max_bitrate_kbps {
            url.push_str(&format!("&maxBitRate={bitrate}"));
        }
        url
    }

    pub(super) fn build_cover_art_url(&self, cover_art_id: &str, size: Option<u32>) -> String {
        let mut url = format!(
            "{}&{}&v={API_VERSION}&c={CLIENT_NAME}",
            self.item_url("getCoverArt.view", cover_art_id),
            self.client.auth()
        );
        if let Some(size) = size.filter(|edge| *edge > 0) {
            url.push_str(&format!("&size={size}"));
        }
        url
    }
}
