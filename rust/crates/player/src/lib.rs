//! Audio playback through libmpv.
//!
//! Events are delivered on mpv's event thread through the callback passed to `Player::new`;
//! the app forwards them to the UI thread.

use feishin_core::queue::Uid;

#[derive(Debug, thiserror::Error)]
pub enum PlayerError {
    #[error("mpv error: {0}")]
    Mpv(String),
}

pub type Result<T> = std::result::Result<T, PlayerError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReplayGainMode {
    #[default]
    Off,
    Track,
    Album,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerConfig {
    /// mpv `audio-device` name; `None` uses the default output.
    pub audio_device: Option<String>,
    pub replay_gain: ReplayGainMode,
    pub gapless: bool,
    /// 0-100.
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

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerEvent {
    /// A track began playing, either from `load` or because a preloaded track took over (gapless).
    Started {
        uid: Uid,
    },
    /// Emitted about every 250 ms while playing.
    Position {
        position_ms: u64,
        duration_ms: Option<u64>,
    },
    Paused,
    Resumed,
    /// The track played to its end (not emitted for `stop` or `load` replacing it).
    TrackEnded {
        uid: Uid,
    },
    Error {
        uid: Option<Uid>,
        message: String,
    },
    /// Nothing left to play.
    Idle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDevice {
    pub name: String,
    pub description: String,
}

pub struct Player {
    _private: (),
}

impl Player {
    pub fn new(
        _config: PlayerConfig,
        _on_event: Box<dyn Fn(PlayerEvent) + Send + Sync>,
    ) -> Result<Self> {
        todo!()
    }

    /// Replace whatever is playing with `url` and start at `start_ms`. Clears any preloaded track.
    pub fn load(&self, _uid: Uid, _url: &str, _start_ms: u64) -> Result<()> {
        todo!()
    }

    /// Queue `url` to start right after the current track (gapless when enabled). Replaces a previous preload.
    pub fn preload_next(&self, _uid: Uid, _url: &str) -> Result<()> {
        todo!()
    }

    pub fn clear_next(&self) -> Result<()> {
        todo!()
    }

    pub fn play(&self) -> Result<()> {
        todo!()
    }

    pub fn pause(&self) -> Result<()> {
        todo!()
    }

    pub fn toggle_pause(&self) -> Result<()> {
        todo!()
    }

    pub fn stop(&self) -> Result<()> {
        todo!()
    }

    pub fn seek(&self, _position_ms: u64) -> Result<()> {
        todo!()
    }

    pub fn set_volume(&self, _volume: u8) -> Result<()> {
        todo!()
    }

    pub fn set_muted(&self, _muted: bool) -> Result<()> {
        todo!()
    }

    pub fn set_replay_gain(&self, _mode: ReplayGainMode) -> Result<()> {
        todo!()
    }

    pub fn set_audio_device(&self, _device: Option<&str>) -> Result<()> {
        todo!()
    }

    pub fn audio_devices(&self) -> Result<Vec<AudioDevice>> {
        todo!()
    }

    pub fn position_ms(&self) -> Option<u64> {
        todo!()
    }
}
