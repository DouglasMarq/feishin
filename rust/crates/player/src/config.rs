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
    pub gapless: bool,
    pub volume: u8,
    pub muted: bool,
    pub crossfade: Duration,
}

impl PlayerConfig {
    pub(crate) fn crossfade_enabled(&self) -> bool {
        !self.crossfade.is_zero()
    }
}

impl Default for PlayerConfig {
    fn default() -> Self {
        Self {
            audio_device: None,
            replay_gain: ReplayGainMode::Off,
            gapless: true,
            volume: DEFAULT_VOLUME,
            muted: false,
            crossfade: Duration::ZERO,
        }
    }
}
