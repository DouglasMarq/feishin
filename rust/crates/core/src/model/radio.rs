use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RadioStation {
    pub id: String,
    pub name: String,
    pub stream_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage_url: Option<String>,
}

const NETWORK_STREAM_SCHEMES: [&str; 8] = [
    "http", "https", "rtsp", "rtmp", "rtmps", "mms", "mmsh", "mmst",
];

impl RadioStation {
    pub fn has_network_stream_url(&self) -> bool {
        let Some((scheme, rest)) = self.stream_url.trim().split_once("://") else {
            return false;
        };
        !rest.is_empty()
            && NETWORK_STREAM_SCHEMES
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(scheme))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RadioStationDraft {
    pub name: String,
    pub stream_url: String,
    pub homepage_url: Option<String>,
}

impl RadioStationDraft {
    pub fn from_station(station: &RadioStation) -> Self {
        Self {
            name: station.name.clone(),
            stream_url: station.stream_url.clone(),
            homepage_url: station.homepage_url.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_url(stream_url: &str) -> RadioStation {
        RadioStation {
            stream_url: stream_url.into(),
            ..RadioStation::default()
        }
    }

    #[test]
    fn only_network_streams_are_playable_station_urls() {
        for playable in [
            "https://jazz.example.com/stream",
            "HTTP://jazz.example.com",
            "rtsp://jazz.example.com/live",
            "mmsh://jazz.example.com/live",
        ] {
            assert!(with_url(playable).has_network_stream_url(), "{playable}");
        }
        for refused in [
            "",
            "https://",
            "file:///home/user/music.flac",
            "ytdl://example",
            "av://lavfi:sine",
            "/etc/passwd",
            "memory://data",
        ] {
            assert!(!with_url(refused).has_network_stream_url(), "{refused}");
        }
    }
}
