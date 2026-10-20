use std::time::Duration;

use anyhow::Result;
use feishin_core::queue::Uid;
use feishin_player::{AudioDevice, MediaSource, Player, PlayerConfig, PlayerEvent, ReplayGainMode};
use feishin_storage::settings::{PlaybackSettings, ReplayGainSetting};

pub type EventCallback = Box<dyn Fn(PlayerEvent) + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub uid: Uid,
    pub url: String,
    pub duration_ms: u64,
}

impl Track {
    pub fn is_live_stream(&self) -> bool {
        self.duration_ms == 0
    }

    fn media_source(&self) -> MediaSource<'_> {
        let source = MediaSource::new(self.uid, &self.url);
        match self.duration_ms {
            0 => source,
            duration_ms => source.with_duration_ms(duration_ms),
        }
    }
}

pub trait AudioOutput: Send {
    fn load(&self, track: &Track, start_ms: u64) -> Result<()>;
    fn preload_next(&self, track: &Track) -> Result<()>;
    fn clear_next(&self) -> Result<()>;
    fn play(&self) -> Result<()>;
    fn pause(&self) -> Result<()>;
    fn stop(&self) -> Result<()>;
    fn seek(&self, position_ms: u64) -> Result<()>;
    fn set_volume(&self, volume: u8) -> Result<()>;
    fn set_muted(&self, muted: bool) -> Result<()>;
    fn set_replay_gain(&self, _mode: ReplayGainSetting) -> Result<()> {
        Ok(())
    }
    fn set_audio_device(&self, _device: Option<&str>) -> Result<()> {
        Ok(())
    }
    fn audio_devices(&self) -> Result<Vec<AudioDevice>> {
        Ok(Vec::new())
    }
    fn set_crossfade(&self, _crossfade: Duration) -> Result<()> {
        Ok(())
    }
}

fn replay_gain_mode(setting: ReplayGainSetting) -> ReplayGainMode {
    match setting {
        ReplayGainSetting::Off => ReplayGainMode::Off,
        ReplayGainSetting::Track => ReplayGainMode::Track,
        ReplayGainSetting::Album => ReplayGainMode::Album,
    }
}

pub struct MpvOutput(Player);

impl MpvOutput {
    pub fn new(settings: &PlaybackSettings, on_event: EventCallback) -> Result<Self> {
        let config = PlayerConfig {
            audio_device: settings.audio_device.clone(),
            replay_gain: replay_gain_mode(settings.replay_gain),
            is_gapless: settings.is_gapless,
            volume: settings.volume_percent,
            is_muted: settings.is_muted,
            crossfade: settings.crossfade(),
        };
        Ok(Self(Player::new(config, on_event)?))
    }
}

impl AudioOutput for MpvOutput {
    fn load(&self, track: &Track, start_ms: u64) -> Result<()> {
        Ok(self.0.load_source(track.media_source(), start_ms)?)
    }
    fn preload_next(&self, track: &Track) -> Result<()> {
        Ok(self.0.preload_source(track.media_source())?)
    }
    fn clear_next(&self) -> Result<()> {
        Ok(self.0.clear_next()?)
    }
    fn play(&self) -> Result<()> {
        Ok(self.0.play()?)
    }
    fn pause(&self) -> Result<()> {
        Ok(self.0.pause()?)
    }
    fn stop(&self) -> Result<()> {
        Ok(self.0.stop()?)
    }
    fn seek(&self, position_ms: u64) -> Result<()> {
        Ok(self.0.seek(position_ms)?)
    }
    fn set_volume(&self, volume: u8) -> Result<()> {
        Ok(self.0.set_volume(volume)?)
    }
    fn set_muted(&self, muted: bool) -> Result<()> {
        Ok(self.0.set_muted(muted)?)
    }
    fn set_replay_gain(&self, mode: ReplayGainSetting) -> Result<()> {
        Ok(self.0.set_replay_gain(replay_gain_mode(mode))?)
    }
    fn set_audio_device(&self, device: Option<&str>) -> Result<()> {
        Ok(self.0.set_audio_device(device)?)
    }
    fn audio_devices(&self) -> Result<Vec<AudioDevice>> {
        Ok(self.0.audio_devices()?)
    }
    fn set_crossfade(&self, crossfade: Duration) -> Result<()> {
        Ok(self.0.set_crossfade(crossfade)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(uid: Uid, duration_ms: u64) -> Track {
        Track {
            uid,
            url: String::new(),
            duration_ms,
        }
    }

    #[test]
    fn media_source_carries_the_known_duration() {
        assert_eq!(track(3, 0).media_source().duration_ms, None);
        assert_eq!(track(3, 1_500).media_source().duration_ms, Some(1_500));
    }
}
