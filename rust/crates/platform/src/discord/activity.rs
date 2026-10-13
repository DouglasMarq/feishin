use discord_rich_presence::activity::{
    Activity, ActivityType, Assets, StatusDisplayType, Timestamps,
};

use super::image_url::is_shareable_image_url;
use super::options::{DiscordDisplayType, DiscordOptions};
use super::text_field::text_field;
use crate::NowPlaying;

const DEFAULT_LARGE_IMAGE: &str = "icon";
const ARTIST_SEPARATOR: &str = ", ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StateIcon {
    Paused,
    Playing,
}

impl StateIcon {
    fn for_playback(paused: bool, options: &DiscordOptions) -> Option<Self> {
        match (paused, options.show_state_icon) {
            (true, _) => Some(StateIcon::Paused),
            (false, true) => Some(StateIcon::Playing),
            (false, false) => None,
        }
    }

    fn image_key(self) -> &'static str {
        match self {
            StateIcon::Paused => "paused",
            StateIcon::Playing => "playing",
        }
    }

    fn text(self) -> &'static str {
        match self {
            StateIcon::Paused => "Paused",
            StateIcon::Playing => "Playing",
        }
    }
}

pub(super) fn activity_for(
    now_playing: &NowPlaying,
    position_ms: Option<u64>,
    paused: bool,
    options: &DiscordOptions,
    now_ms: i64,
) -> Option<Activity<'static>> {
    if now_playing.track_id.is_empty() || (paused && !options.show_paused) {
        return None;
    }
    let activity = Activity::new()
        .details(text_field(&now_playing.title, "Idle"))
        .state(text_field(
            &now_playing.artists.join(ARTIST_SEPARATOR),
            "Unknown artist",
        ))
        .assets(assets_for(
            now_playing,
            StateIcon::for_playback(paused, options),
        ))
        .activity_type(activity_type_for(options))
        .status_display_type(status_display_type_for(options.display_type));
    let Some(position_ms) = position_ms.filter(|_| !paused) else {
        return Some(activity);
    };
    Some(activity.timestamps(timestamps_for(now_playing, position_ms, now_ms)))
}

fn assets_for(now_playing: &NowPlaying, state_icon: Option<StateIcon>) -> Assets<'static> {
    let assets = Assets::new()
        .large_image(large_image_for(now_playing))
        .large_text(text_field(
            now_playing.album.as_deref().unwrap_or_default(),
            "Unknown album",
        ));
    match state_icon {
        Some(icon) => assets.small_image(icon.image_key()).small_text(icon.text()),
        None => assets,
    }
}

fn large_image_for(now_playing: &NowPlaying) -> String {
    now_playing
        .cover_url
        .as_deref()
        .filter(|url| is_shareable_image_url(url))
        .unwrap_or(DEFAULT_LARGE_IMAGE)
        .to_owned()
}

fn activity_type_for(options: &DiscordOptions) -> ActivityType {
    match options.show_as_listening {
        true => ActivityType::Listening,
        false => ActivityType::Playing,
    }
}

fn status_display_type_for(display_type: DiscordDisplayType) -> StatusDisplayType {
    match display_type {
        DiscordDisplayType::Feishin => StatusDisplayType::Name,
        DiscordDisplayType::ArtistName => StatusDisplayType::State,
        DiscordDisplayType::SongName => StatusDisplayType::Details,
    }
}

fn timestamps_for(now_playing: &NowPlaying, position_ms: u64, now_ms: i64) -> Timestamps {
    let start = now_ms.saturating_sub(i64::try_from(position_ms).unwrap_or(i64::MAX));
    let timestamps = Timestamps::new().start(start);
    let duration_ms = now_playing
        .duration_ms
        .filter(|duration_ms| *duration_ms > 0)
        .and_then(|duration_ms| i64::try_from(duration_ms).ok());
    match duration_ms {
        Some(duration_ms) => timestamps.end(start.saturating_add(duration_ms)),
        None => timestamps,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    const NOW: i64 = 1_700_000_000_000;

    fn song() -> NowPlaying {
        NowPlaying {
            track_id: "t1".to_owned(),
            title: "Song".to_owned(),
            artists: vec!["Artist A".to_owned(), "Artist B".to_owned()],
            album: Some("Album".to_owned()),
            duration_ms: Some(200_000),
            cover_url: Some("https://example.com/cover.jpg".to_owned()),
        }
    }

    fn render(
        now_playing: &NowPlaying,
        position_ms: Option<u64>,
        paused: bool,
        options: &DiscordOptions,
    ) -> Option<Value> {
        activity_for(now_playing, position_ms, paused, options, NOW)
            .map(|activity| serde_json::to_value(activity).unwrap())
    }

    #[test]
    fn playing_activity() {
        let value = render(&song(), Some(30_000), false, &DiscordOptions::default()).unwrap();
        assert_eq!(
            value,
            json!({
                "details": "Song",
                "state": "Artist A, Artist B",
                "type": 2,
                "status_display_type": 0,
                "assets": {
                    "large_image": "https://example.com/cover.jpg",
                    "large_text": "Album",
                    "small_image": "playing",
                    "small_text": "Playing",
                },
                "timestamps": {
                    "start": NOW - 30_000,
                    "end": NOW - 30_000 + 200_000,
                },
            })
        );
    }

    #[test]
    fn paused_activity_has_no_timestamps() {
        let value = render(&song(), Some(30_000), true, &DiscordOptions::default()).unwrap();
        assert!(value.get("timestamps").is_none());
        assert_eq!(value["assets"]["small_image"], "paused");
        assert_eq!(value["assets"]["small_text"], "Paused");
    }

    #[test]
    fn paused_is_cleared_when_hidden() {
        let options = DiscordOptions {
            show_paused: false,
            ..DiscordOptions::default()
        };
        assert!(render(&song(), Some(0), true, &options).is_none());
        assert!(render(&song(), Some(0), false, &options).is_some());
    }

    #[test]
    fn no_track_clears() {
        assert!(
            render(
                &NowPlaying::default(),
                None,
                false,
                &DiscordOptions::default()
            )
            .is_none()
        );
    }

    #[test]
    fn fallbacks_and_padding() {
        let now_playing = NowPlaying {
            track_id: "t1".to_owned(),
            title: "A".to_owned(),
            duration_ms: None,
            ..NowPlaying::default()
        };
        let options = DiscordOptions {
            show_state_icon: false,
            show_as_listening: false,
            display_type: DiscordDisplayType::SongName,
            ..DiscordOptions::default()
        };
        let value = render(&now_playing, Some(1_000), false, &options).unwrap();
        assert_eq!(value["details"], "A ");
        assert_eq!(value["state"], "Unknown artist");
        assert_eq!(value["type"], 0);
        assert_eq!(value["status_display_type"], 2);
        assert_eq!(
            value["assets"],
            json!({ "large_image": "icon", "large_text": "Unknown album" })
        );
        assert_eq!(value["timestamps"], json!({ "start": NOW - 1_000 }));
    }
}
