use feishin_platform::NowPlaying;

use super::RadioPlayback;
use crate::controller::Controller;

const RADIO_TRACK_PREFIX: &str = "radio-";

fn radio_now_playing(radio: &RadioPlayback) -> NowPlaying {
    let station_name = radio.station.name.clone();
    let (title, artists) = match radio.stream_title.clone() {
        Some(stream_title) => (stream_title, vec![station_name]),
        None => (station_name, vec![tr!("Internet radio")]),
    };
    NowPlaying {
        track_id: format!("{RADIO_TRACK_PREFIX}{}", radio.station.id),
        title,
        artists,
        album: Some(radio.station.name.clone()),
        duration_ms: None,
        cover_url: None,
    }
}

impl Controller {
    pub(super) fn update_radio_media(&mut self) {
        let Some(now_playing) = self.radio.as_ref().map(radio_now_playing) else {
            return;
        };
        if let Err(error) = self.deps.media.set_metadata(&now_playing) {
            tracing::debug!("media session radio metadata: {error:#}");
        }
        self.update_media_status();
    }

    pub(super) fn set_media_live_stream(&mut self, live: bool) {
        if let Err(error) = self.deps.media.set_live_stream(live) {
            tracing::debug!("media session live stream: {error:#}");
        }
    }
}

#[cfg(test)]
mod tests {
    use feishin_core::model::RadioStation;

    use super::*;

    fn playback(stream_title: Option<&str>) -> RadioPlayback {
        RadioPlayback {
            station: RadioStation {
                id: "7".into(),
                name: "Jazz FM".into(),
                stream_url: "https://jazz.example.com/stream".into(),
                homepage_url: None,
            },
            stream_title: stream_title.map(str::to_owned),
            elapsed_ms: 0,
        }
    }

    #[test]
    fn the_station_is_the_title_until_a_stream_title_arrives() {
        let waiting = radio_now_playing(&playback(None));
        assert_eq!(waiting.track_id, "radio-7");
        assert_eq!(waiting.title, "Jazz FM");
        assert_eq!(waiting.artists, ["Internet radio"]);
        assert_eq!(waiting.duration_ms, None);

        let titled = radio_now_playing(&playback(Some("Artist - Song")));
        assert_eq!(titled.title, "Artist - Song");
        assert_eq!(titled.artists, ["Jazz FM"]);
        assert_eq!(titled.album.as_deref(), Some("Jazz FM"));
    }
}
