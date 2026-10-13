use feishin_api::StreamOptions;

use super::JellyfinServer;

const DEFAULT_TRANSCODE_FORMAT: &str = "mp3";
const BITS_PER_KILOBIT: u64 = 1000;

impl JellyfinServer {
    pub(super) fn song_stream_url(&self, song_id: &str, options: &StreamOptions) -> String {
        match (&options.format, options.max_bitrate_kbps) {
            (None, None) => self.direct_download_url(song_id),
            _ => self.transcoding_url(song_id, options),
        }
    }

    pub(super) fn primary_image_url(&self, cover_art_id: &str, edge_pixels: Option<u32>) -> String {
        let image_url = format!(
            "{}/Items/{cover_art_id}/Images/Primary?quality=96",
            self.base_url
        );
        let size_suffix = edge_pixels
            .map(|edge| format!("&fillHeight={edge}&fillWidth={edge}"))
            .unwrap_or_default();
        format!("{image_url}{size_suffix}")
    }

    fn direct_download_url(&self, song_id: &str) -> String {
        format!(
            "{}/Items/{song_id}/Download?apiKey={}&playSessionId=",
            self.base_url, self.access_token
        )
    }

    fn transcoding_url(&self, song_id: &str, options: &StreamOptions) -> String {
        let (base, token, user_id) = (&self.base_url, &self.access_token, &self.user_id);
        let format = options
            .format
            .as_deref()
            .unwrap_or(DEFAULT_TRANSCODE_FORMAT);
        let session = format!("feishin-{song_id}-{}-0-0", format.to_lowercase());
        let universal_url = format!(
            "{base}/audio/{song_id}/universal?userId={user_id}&deviceId=&audioCodec={format}&apiKey={token}\
             &playSessionId={session}&container={format}&transcodingProtocol=http\
             &transcodingContainer={format}"
        );
        let bitrate_suffix = options
            .max_bitrate_kbps
            .map(|kbps| {
                format!(
                    "&maxStreamingBitrate={}",
                    u64::from(kbps) * BITS_PER_KILOBIT
                )
            })
            .unwrap_or_default();
        format!("{universal_url}{bitrate_suffix}")
    }
}
