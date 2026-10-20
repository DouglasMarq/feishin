use std::time::Duration;

use feishin_core::DEFAULT_VOLUME;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReplayGainMode {
    #[default]
    Off,
    Track,
    Album,
}

impl ReplayGainMode {
    pub(crate) fn mpv_value(self) -> &'static str {
        match self {
            ReplayGainMode::Off => "no",
            ReplayGainMode::Track => "track",
            ReplayGainMode::Album => "album",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerConfig {
    pub audio_device: Option<String>,
    pub replay_gain: ReplayGainMode,
    pub is_gapless: bool,
    pub volume: u8,
    pub is_muted: bool,
    pub crossfade: Duration,
}

impl PlayerConfig {
    pub(crate) fn is_crossfade_enabled(&self) -> bool {
        !self.crossfade.is_zero()
    }
}

impl Default for PlayerConfig {
    fn default() -> Self {
        Self {
            audio_device: None,
            replay_gain: ReplayGainMode::Off,
            is_gapless: true,
            volume: DEFAULT_VOLUME,
            is_muted: false,
            crossfade: Duration::ZERO,
        }
    }
}
