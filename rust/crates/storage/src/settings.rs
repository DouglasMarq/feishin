mod lyrics_source;
mod youtube;

use std::time::Duration;

use feishin_api::TranscodeFormat;
use feishin_core::DEFAULT_VOLUME;
use feishin_core::model::{RadioStation, ServerConfig};
use serde::{Deserialize, Deserializer, Serialize};

use crate::appearance::AppearanceSettings;
use crate::{Paths, Result, StorageError, none_when_missing, write_atomic};

pub use lyrics_source::LyricsSourceSetting;
pub use youtube::YouTubeSettings;

pub const SETTINGS_FILE: &str = "settings.toml";
pub const MAX_CROSSFADE_SECONDS: u8 = 12;
const DEFAULT_COLUMN_WIDTH: f32 = 100.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ReplayGainSetting {
    #[default]
    Off,
    Track,
    Album,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlaybackSettings {
    #[serde(rename = "volume")]
    pub volume_percent: u8,
    pub muted: bool,
    pub replay_gain: ReplayGainSetting,
    pub gapless: bool,
    pub audio_device: Option<String>,
    pub max_bitrate_kbps: Option<u32>,
    #[serde(deserialize_with = "known_transcode_format")]
    pub transcode_format: Option<TranscodeFormat>,
    pub scrobble: bool,
    pub crossfade_seconds: u8,
}

impl PlaybackSettings {
    pub fn crossfade(&self) -> Duration {
        Duration::from_secs(u64::from(self.crossfade_seconds.min(MAX_CROSSFADE_SECONDS)))
    }
}

fn known_transcode_format<'de, FormatDeserializer: Deserializer<'de>>(
    deserializer: FormatDeserializer,
) -> std::result::Result<Option<TranscodeFormat>, FormatDeserializer::Error> {
    let name = Option::<String>::deserialize(deserializer)?;
    Ok(name.and_then(|name| {
        let format = TranscodeFormat::parse(&name);
        if format.is_none() {
            tracing::warn!("ignoring the invalid transcode format {name}");
        }
        format
    }))
}

impl Default for PlaybackSettings {
    fn default() -> Self {
        Self {
            volume_percent: DEFAULT_VOLUME,
            muted: false,
            replay_gain: ReplayGainSetting::Off,
            gapless: true,
            audio_device: None,
            max_bitrate_kbps: None,
            transcode_format: None,
            scrobble: true,
            crossfade_seconds: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnSetting {
    pub id: String,
    #[serde(rename = "width", default = "default_column_width")]
    pub width_pixels: f32,
    #[serde(default = "visible_by_default")]
    pub visible: bool,
}

fn default_column_width() -> f32 {
    DEFAULT_COLUMN_WIDTH
}

fn visible_by_default() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSettings {
    pub track_columns: Vec<ColumnSetting>,
    #[serde(rename = "sidebar_width")]
    pub sidebar_width_pixels: Option<f32>,
    pub queue_panel_open: bool,
    pub follow_current_song: bool,
    pub language: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IntegrationSettings {
    pub mpris: bool,
    pub discord_rich_presence: bool,
    pub discord_client_id: Option<String>,
    #[serde(deserialize_with = "lyrics_source::known_lyrics_sources")]
    pub lyrics_sources: Vec<LyricsSourceSetting>,
    pub remote_control: bool,
}

impl Default for IntegrationSettings {
    fn default() -> Self {
        Self {
            mpris: true,
            discord_rich_presence: false,
            discord_client_id: None,
            lyrics_sources: vec![
                LyricsSourceSetting::Server,
                LyricsSourceSetting::Lrclib,
                LyricsSourceSetting::NetEase,
            ],
            remote_control: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RadioSettings {
    pub favorites: Vec<RadioStation>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub servers: Vec<ServerConfig>,
    pub current_server_id: Option<String>,
    pub playback: PlaybackSettings,
    pub ui: UiSettings,
    pub integrations: IntegrationSettings,
    pub appearance: AppearanceSettings,
    pub radio: RadioSettings,
    pub youtube: YouTubeSettings,
}

impl Settings {
    pub fn load(paths: &Paths) -> Result<Self> {
        let settings_path = paths.config_dir.join(SETTINGS_FILE);
        let Some(text) = none_when_missing(std::fs::read_to_string(settings_path))? else {
            return Ok(Self::default());
        };
        toml::from_str(&text).map_err(invalid_settings_file)
    }

    pub fn save(&self, paths: &Paths) -> Result<()> {
        let text = toml::to_string_pretty(self).map_err(invalid_settings_file)?;
        write_atomic(&paths.config_dir.join(SETTINGS_FILE), text.as_bytes())
    }
}

fn invalid_settings_file(error: impl std::error::Error + Send + Sync + 'static) -> StorageError {
    StorageError::malformed(SETTINGS_FILE, error)
}

#[cfg(test)]
mod tests {
    use feishin_core::model::ServerType;

    use super::*;

    fn temp_paths() -> (tempfile::TempDir, Paths) {
        let directory = tempfile::tempdir().unwrap();
        let paths = Paths::under(directory.path());
        (directory, paths)
    }

    fn sample() -> Settings {
        Settings {
            servers: vec![ServerConfig {
                id: "abc".into(),
                name: "Home".into(),
                server_type: ServerType::Navidrome,
                url: "https://music.example.com".into(),
                username: "me".into(),
            }],
            current_server_id: Some("abc".into()),
            playback: PlaybackSettings {
                volume_percent: 42,
                replay_gain: ReplayGainSetting::Album,
                audio_device: Some("pulse/sink".into()),
                max_bitrate_kbps: Some(320),
                transcode_format: TranscodeFormat::parse("opus"),
                crossfade_seconds: 5,
                ..Default::default()
            },
            ui: UiSettings {
                track_columns: vec![ColumnSetting {
                    id: "title".into(),
                    width_pixels: 250.5,
                    visible: true,
                }],
                sidebar_width_pixels: Some(220.0),
                queue_panel_open: true,
                follow_current_song: true,
                language: Some("pt-BR".into()),
            },
            integrations: IntegrationSettings {
                discord_rich_presence: true,
                lyrics_sources: vec![LyricsSourceSetting::Genius, LyricsSourceSetting::Server],
                ..Default::default()
            },
            appearance: AppearanceSettings::default(),
            radio: sample_radio(),
            youtube: sample_youtube(),
        }
    }

    fn sample_youtube() -> YouTubeSettings {
        YouTubeSettings {
            yt_dlp_path: Some("/opt/yt-dlp".into()),
            cookies_from_browser: Some(feishin_youtube::CookieBrowser::Firefox),
            country: Some("BR".into()),
        }
    }

    fn sample_radio() -> RadioSettings {
        RadioSettings {
            favorites: vec![RadioStation {
                id: "station-1".into(),
                name: "Smooth Jazz".into(),
                stream_url: "https://jazz.example.com/stream".into(),
                homepage_url: None,
            }],
        }
    }

    #[test]
    fn missing_file_gives_defaults() {
        let (_directory, paths) = temp_paths();
        assert_eq!(Settings::load(&paths).unwrap(), Settings::default());
    }

    #[test]
    fn round_trip() {
        let (_directory, paths) = temp_paths();
        let settings = sample();
        settings.save(&paths).unwrap();
        assert_eq!(Settings::load(&paths).unwrap(), settings);
    }

    #[test]
    fn save_overwrites_and_leaves_only_the_file() {
        let (_directory, paths) = temp_paths();
        sample().save(&paths).unwrap();
        Settings::default().save(&paths).unwrap();
        assert_eq!(Settings::load(&paths).unwrap(), Settings::default());
        let names: Vec<_> = std::fs::read_dir(&paths.config_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names, vec![SETTINGS_FILE.to_string()]);
    }

    #[test]
    fn tolerates_unknown_and_missing_fields() {
        let (_directory, paths) = temp_paths();
        std::fs::create_dir_all(&paths.config_dir).unwrap();
        std::fs::write(
            paths.config_dir.join(SETTINGS_FILE),
            r#"
current_server_id = "x"
future_top_level = 1

[playback]
volume = 10
something_new = "yes"

[[ui.track_columns]]
id = "artist"

[future_section]
a = 1
"#,
        )
        .unwrap();
        let settings = Settings::load(&paths).unwrap();
        assert_eq!(settings.current_server_id.as_deref(), Some("x"));
        assert_eq!(settings.playback.volume_percent, 10);
        assert!(settings.playback.gapless);
        assert!(settings.playback.scrobble);
        assert_eq!(settings.playback.crossfade(), Duration::ZERO);
        assert_eq!(
            settings.ui.track_columns,
            vec![ColumnSetting {
                id: "artist".into(),
                width_pixels: 100.0,
                visible: true,
            }]
        );
        assert_eq!(settings.integrations, IntegrationSettings::default());
        assert_eq!(settings.radio, RadioSettings::default());
        assert_eq!(settings.youtube, YouTubeSettings::default());
    }

    const PREVIOUS_RELEASE_SETTINGS: &str = r#"
current_server_id = "abc"

[[servers]]
id = "abc"
name = "Home"
server_type = "Navidrome"
url = "https://music.example.com"
username = "me"

[playback]
volume = 42
muted = false
replay_gain = "Album"
gapless = true
max_bitrate_kbps = 320
transcode_format = "opus"
scrobble = true
crossfade_seconds = 5

[ui]
sidebar_width = 220.0

[[ui.track_columns]]
id = "title"
width = 250.5
visible = true

[[ui.track_columns]]
id = "plays"
width = 64.0
visible = false

[integrations]
mpris = true
discord_rich_presence = false
lyrics_sources = ["genius", "server", "NetEase"]
remote_control = false
"#;

    fn write_settings(paths: &Paths, text: &str) {
        std::fs::create_dir_all(&paths.config_dir).unwrap();
        std::fs::write(paths.config_dir.join(SETTINGS_FILE), text).unwrap();
    }

    #[test]
    fn given_a_settings_file_from_the_previous_release_it_loads_unchanged() {
        let (_directory, paths) = temp_paths();
        write_settings(&paths, PREVIOUS_RELEASE_SETTINGS);

        let settings = Settings::load(&paths).unwrap();

        assert_eq!(settings.servers, sample().servers);
        assert_eq!(settings.playback.replay_gain, ReplayGainSetting::Album);
        assert_eq!(
            settings.playback.transcode_format,
            TranscodeFormat::parse("opus")
        );
        assert_eq!(
            settings.ui.track_columns[1],
            ColumnSetting {
                id: "plays".into(),
                width_pixels: 64.0,
                visible: false,
            }
        );
        assert_eq!(
            settings.integrations.lyrics_sources,
            [
                LyricsSourceSetting::Genius,
                LyricsSourceSetting::Server,
                LyricsSourceSetting::NetEase
            ]
        );
    }

    #[test]
    fn given_typed_settings_they_are_saved_with_the_previous_release_names() {
        let (_directory, paths) = temp_paths();
        write_settings(&paths, PREVIOUS_RELEASE_SETTINGS);
        Settings::load(&paths).unwrap().save(&paths).unwrap();

        let saved = std::fs::read_to_string(paths.config_dir.join(SETTINGS_FILE)).unwrap();

        assert!(saved.contains(r#"transcode_format = "opus""#), "{saved}");
        assert!(saved.contains(r#""genius","#), "{saved}");
        assert!(saved.contains(r#""netease","#), "{saved}");
        assert!(saved.contains(r#"id = "plays""#), "{saved}");
    }

    #[test]
    fn given_a_previous_release_file_its_volume_and_widths_load_into_the_renamed_fields() {
        let (_directory, paths) = temp_paths();
        write_settings(&paths, PREVIOUS_RELEASE_SETTINGS);

        let settings = Settings::load(&paths).unwrap();

        assert_eq!(settings.playback.volume_percent, 42);
        assert_eq!(settings.ui.sidebar_width_pixels, Some(220.0));
        assert_eq!(settings.ui.track_columns[0].width_pixels, 250.5);
    }

    #[test]
    fn given_renamed_volume_and_widths_they_are_saved_with_the_previous_release_names() {
        let (_directory, paths) = temp_paths();
        write_settings(&paths, PREVIOUS_RELEASE_SETTINGS);
        Settings::load(&paths).unwrap().save(&paths).unwrap();

        let saved = std::fs::read_to_string(paths.config_dir.join(SETTINGS_FILE)).unwrap();

        assert!(saved.contains("volume = 42\n"), "{saved}");
        assert!(saved.contains("sidebar_width = 220.0\n"), "{saved}");
        assert!(saved.contains("width = 250.5\n"), "{saved}");
        assert!(!saved.contains("_pixels"), "{saved}");
        assert!(!saved.contains("_percent"), "{saved}");
    }

    #[test]
    fn given_an_unknown_lyrics_source_and_format_they_are_dropped_without_failing() {
        let (_directory, paths) = temp_paths();
        write_settings(
            &paths,
            r#"
[playback]
transcode_format = "mp3&t=stolen"

[integrations]
lyrics_sources = ["musixmatch", "lrclib"]
"#,
        );

        let settings = Settings::load(&paths).unwrap();

        assert_eq!(settings.playback.transcode_format, None);
        assert_eq!(
            settings.integrations.lyrics_sources,
            [LyricsSourceSetting::Lrclib]
        );
    }

    #[test]
    fn crossfade_is_capped_at_twelve_seconds() {
        let playback = |crossfade_seconds| PlaybackSettings {
            crossfade_seconds,
            ..PlaybackSettings::default()
        };
        assert_eq!(playback(0).crossfade(), Duration::ZERO);
        assert_eq!(playback(7).crossfade(), Duration::from_secs(7));
        assert_eq!(playback(200).crossfade(), Duration::from_secs(12));
    }

    #[test]
    fn invalid_toml_is_an_error() {
        let (_directory, paths) = temp_paths();
        std::fs::create_dir_all(&paths.config_dir).unwrap();
        std::fs::write(paths.config_dir.join(SETTINGS_FILE), "volume = [").unwrap();
        assert!(matches!(
            Settings::load(&paths),
            Err(StorageError::Malformed { .. })
        ));
    }
}
