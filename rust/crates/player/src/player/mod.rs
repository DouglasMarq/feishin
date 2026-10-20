mod startup;

use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use feishin_core::queue::Uid;

use crate::error::Result;
use crate::mixer::Mixer;
use crate::{AudioDevice, MediaSource, ReplayGainMode};

pub struct Player {
    mixer: Arc<Mixer>,
    fader_thread: Option<JoinHandle<()>>,
}

impl Player {
    pub fn load(&self, uid: Uid, url: &str, start_ms: u64) -> Result<()> {
        self.load_source(MediaSource::new(uid, url), start_ms)
    }

    pub fn load_source(&self, source: MediaSource<'_>, start_ms: u64) -> Result<()> {
        self.mixer.update(|state| state.load(source, start_ms))
    }

    pub fn preload_next(&self, uid: Uid, url: &str) -> Result<()> {
        self.preload_source(MediaSource::new(uid, url))
    }

    pub fn preload_source(&self, source: MediaSource<'_>) -> Result<()> {
        self.mixer.update(|state| state.preload_next(source))
    }

    pub fn clear_next(&self) -> Result<()> {
        self.mixer.update(|state| state.clear_next())
    }

    pub fn stop(&self) -> Result<()> {
        self.mixer.update(|state| state.stop())
    }

    pub fn play(&self) -> Result<()> {
        self.mixer.update(|state| state.set_paused(false))
    }

    pub fn pause(&self) -> Result<()> {
        self.mixer.update(|state| state.set_paused(true))
    }

    pub fn toggle_pause(&self) -> Result<()> {
        self.mixer.update(|state| state.toggle_pause())
    }

    pub fn seek(&self, position_ms: u64) -> Result<()> {
        self.mixer.update(|state| state.seek(position_ms))
    }

    pub fn set_volume(&self, volume: u8) -> Result<()> {
        self.mixer.update(|state| state.set_volume(volume))
    }

    pub fn set_muted(&self, is_muted: bool) -> Result<()> {
        self.mixer.update(|state| state.set_muted(is_muted))
    }

    pub fn set_replay_gain(&self, mode: ReplayGainMode) -> Result<()> {
        self.mixer.update(|state| state.set_replay_gain(mode))
    }

    pub fn set_audio_device(&self, device: Option<&str>) -> Result<()> {
        self.mixer.update(|state| state.set_audio_device(device))
    }

    pub fn set_crossfade(&self, crossfade: Duration) -> Result<()> {
        self.mixer.set_crossfade(crossfade)
    }

    pub fn audio_devices(&self) -> Result<Vec<AudioDevice>> {
        self.mixer.read(|state| state.audio_devices())
    }

    pub fn position_ms(&self) -> Option<u64> {
        self.mixer.read(|state| state.position_ms())
    }

    #[doc(hidden)]
    pub fn is_crossfading(&self) -> bool {
        self.mixer.read(|state| state.is_crossfading())
    }

    #[doc(hidden)]
    pub fn holds_next_track(&self) -> bool {
        self.mixer.read(|state| state.holds_next_track())
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let decks = self.mixer.shut_down();
        if let Some(fader_thread) = self.fader_thread.take()
            && fader_thread.thread().id() != thread::current().id()
        {
            fader_thread.join().ok();
        }
        drop(decks);
    }
}
