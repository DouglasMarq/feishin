use super::state::MixerState;
use crate::deck::{Deck, DeckSetting, MAX_VOLUME};
use crate::error::Result;
use crate::time::seconds_to_milliseconds;
use crate::{AudioDevice, ReplayGainMode};

impl MixerState {
    pub(crate) fn set_volume(&mut self, volume: u8) -> Result<()> {
        self.config.volume = volume.min(MAX_VOLUME);
        self.apply_to_every_deck(&DeckSetting::Volume(volume))
    }

    pub(crate) fn set_muted(&mut self, muted: bool) -> Result<()> {
        self.config.muted = muted;
        self.apply_to_every_deck(&DeckSetting::Muted(muted))
    }

    pub(crate) fn set_replay_gain(&mut self, mode: ReplayGainMode) -> Result<()> {
        self.config.replay_gain = mode;
        self.apply_to_every_deck(&DeckSetting::ReplayGain(mode))
    }

    pub(crate) fn set_audio_device(&mut self, device: Option<&str>) -> Result<()> {
        self.config.audio_device = device.map(str::to_owned);
        self.apply_to_every_deck(&DeckSetting::AudioDevice(self.config.audio_device.clone()))
    }

    pub(crate) fn audio_devices(&self) -> Result<Vec<AudioDevice>> {
        self.lead_deck()?.audio_devices()
    }

    pub(crate) fn position_ms(&self) -> Option<u64> {
        self.lead_deck()
            .ok()
            .and_then(Deck::position_seconds)
            .map(seconds_to_milliseconds)
    }

    fn apply_to_every_deck(&self, setting: &DeckSetting) -> Result<()> {
        self.apply_to_standby(setting);
        self.lead_deck()?.apply(setting)
    }
}
