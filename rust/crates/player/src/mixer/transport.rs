use super::state::{MixerState, PendingTrack};
use crate::MediaSource;
use crate::crossfade::Gain;
use crate::deck::DeckSetting;
use crate::error::{Result, log_failure};

impl MixerState {
    pub(crate) fn load(&mut self, source: MediaSource<'_>, start_ms: u64) -> Result<()> {
        self.pending_next = None;
        self.dismiss_standby();
        self.durations.remember(source.uid, source.duration_ms);
        self.durations.playing(self.routing.lead(), source.uid);
        let lead = self.lead_deck()?;
        log_failure(
            lead.apply(&DeckSetting::Gain(Gain::UNITY)),
            "restoring the gain",
        );
        lead.replace(source.uid, source.url, start_ms)?;
        self.set_lead_paused(false)
    }

    pub(crate) fn preload_next(&mut self, source: MediaSource<'_>) -> Result<()> {
        self.durations.remember(source.uid, source.duration_ms);
        if !self.config.is_crossfade_enabled() {
            return self.lead_deck()?.append_next(source.uid, source.url);
        }
        self.pending_next = Some(PendingTrack {
            uid: source.uid,
            url: source.url.to_owned(),
        });
        match self.routing.is_fading() {
            true => Ok(()),
            false => self.load_standby_when_due(),
        }
    }

    pub(crate) fn clear_next(&mut self) -> Result<()> {
        self.pending_next = None;
        if !self.routing.is_fading() {
            self.dismiss_standby();
        }
        self.lead_deck()?.clear_next()
    }

    pub(crate) fn stop(&mut self) -> Result<()> {
        self.pending_next = None;
        self.dismiss_standby();
        self.lead_deck()?.stop()
    }

    pub(crate) fn seek(&mut self, position_ms: u64) -> Result<()> {
        self.finish_fade();
        self.lead_deck()?.seek(position_ms)
    }

    pub(crate) fn set_paused(&mut self, is_paused: bool) -> Result<()> {
        if self.routing.is_fading() {
            self.apply_to_standby(&DeckSetting::Paused(is_paused));
        }
        self.set_lead_paused(is_paused)
    }

    pub(crate) fn toggle_pause(&mut self) -> Result<()> {
        let is_paused = self.lead_deck()?.is_paused();
        self.set_paused(!is_paused)
    }
}
