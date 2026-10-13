use feishin_core::model::ServerConfig;
use serde::{Deserialize, Serialize};

use crate::{Paths, Result, StorageError, write_atomic};

pub const SETTINGS_FILE: &str = "settings.toml";

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
    pub volume: u8,
    pub muted: bool,
    pub replay_gain: ReplayGainSetting,
    pub gapless: bool,
    pub audio_device: Option<String>,
    pub max_bitrate_kbps: Option<u32>,
    pub transcode_format: Option<String>,
    pub scrobble: bool,
}

impl Default for PlaybackSettings {
    fn default() -> Self {
        Self {
            volume: 70,
            muted: false,
            replay_gain: ReplayGainSetting::Off,
            gapless: true,
            audio_device: None,
            max_bitrate_kbps: None,
            transcode_format: None,
            scrobble: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnSetting {
    pub id: String,
    #[serde(default = "default_column_width")]
    pub width: f32,
    #[serde(default = "default_true")]
    pub visible: bool,
}

fn default_column_width() -> f32 {
    100.0
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSettings {
    pub track_columns: Vec<ColumnSetting>,
    pub sidebar_width: Option<f32>,
    pub queue_panel_open: bool,
    pub follow_current_song: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct IntegrationSettings {
    pub mpris: bool,
    pub discord_rich_presence: bool,
    pub discord_client_id: Option<String>,
    /// Lyrics providers in priority order, e.g. `["server", "lrclib", "netease", "genius"]`.
    pub lyrics_sources: Vec<String>,
}

impl Default for IntegrationSettings {
    fn default() -> Self {
        Self {
            mpris: true,
            discord_rich_presence: false,
            discord_client_id: None,
            lyrics_sources: vec!["server".into(), "lrclib".into(), "netease".into()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub servers: Vec<ServerConfig>,
    pub current_server_id: Option<String>,
    pub playback: PlaybackSettings,
    pub ui: UiSettings,
    pub integrations: IntegrationSettings,
}

impl Settings {
    /// Loads `settings.toml` from the config dir; returns defaults when the file does not exist.
    pub fn load(paths: &Paths) -> Result<Self> {
        let text = match std::fs::read_to_string(paths.config_dir.join(SETTINGS_FILE)) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e.into()),
        };
        toml::from_str(&text).map_err(|e| StorageError::Invalid(format!("{SETTINGS_FILE}: {e}")))
    }

    /// Writes atomically (temp file + rename).
    pub fn save(&self, paths: &Paths) -> Result<()> {
        let text = toml::to_string_pretty(self)
            .map_err(|e| StorageError::Invalid(format!("{SETTINGS_FILE}: {e}")))?;
        write_atomic(&paths.config_dir.join(SETTINGS_FILE), text.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use feishin_core::model::ServerType;

    use super::*;

    fn temp_paths() -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path());
        (dir, paths)
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
                volume: 42,
                replay_gain: ReplayGainSetting::Album,
                audio_device: Some("pulse/sink".into()),
                max_bitrate_kbps: Some(320),
                ..Default::default()
            },
            ui: UiSettings {
                track_columns: vec![ColumnSetting {
                    id: "title".into(),
                    width: 250.5,
                    visible: true,
                }],
                sidebar_width: Some(220.0),
                queue_panel_open: true,
                follow_current_song: true,
            },
            integrations: IntegrationSettings {
                discord_rich_presence: true,
                ..Default::default()
            },
        }
    }

    #[test]
    fn missing_file_gives_defaults() {
        let (_dir, paths) = temp_paths();
        assert_eq!(Settings::load(&paths).unwrap(), Settings::default());
    }

    #[test]
    fn round_trip() {
        let (_dir, paths) = temp_paths();
        let settings = sample();
        settings.save(&paths).unwrap();
        assert_eq!(Settings::load(&paths).unwrap(), settings);
    }

    #[test]
    fn save_overwrites_and_leaves_only_the_file() {
        let (_dir, paths) = temp_paths();
        sample().save(&paths).unwrap();
        Settings::default().save(&paths).unwrap();
        assert_eq!(Settings::load(&paths).unwrap(), Settings::default());
        let names: Vec<_> = std::fs::read_dir(&paths.config_dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names, vec![SETTINGS_FILE.to_string()]);
    }

    #[test]
    fn tolerates_unknown_and_missing_fields() {
        let (_dir, paths) = temp_paths();
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
        assert_eq!(settings.playback.volume, 10);
        assert!(settings.playback.gapless);
        assert!(settings.playback.scrobble);
        assert_eq!(
            settings.ui.track_columns,
            vec![ColumnSetting {
                id: "artist".into(),
                width: 100.0,
                visible: true,
            }]
        );
        assert_eq!(settings.integrations, IntegrationSettings::default());
    }

    #[test]
    fn invalid_toml_is_an_error() {
        let (_dir, paths) = temp_paths();
        std::fs::create_dir_all(&paths.config_dir).unwrap();
        std::fs::write(paths.config_dir.join(SETTINGS_FILE), "volume = [").unwrap();
        assert!(matches!(
            Settings::load(&paths),
            Err(StorageError::Invalid(_))
        ));
    }
}
