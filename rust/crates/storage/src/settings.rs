use feishin_core::model::ServerConfig;
use serde::{Deserialize, Serialize};

use crate::{Paths, Result};

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
    pub width: f32,
    pub visible: bool,
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
    pub fn load(_paths: &Paths) -> Result<Self> {
        todo!()
    }

    /// Writes atomically (temp file + rename).
    pub fn save(&self, _paths: &Paths) -> Result<()> {
        todo!()
    }
}
