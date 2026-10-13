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
}

impl Default for PlayerConfig {
    fn default() -> Self {
        Self {
            audio_device: None,
            replay_gain: ReplayGainMode::Off,
            gapless: true,
            volume: 70,
        }
    }
}
