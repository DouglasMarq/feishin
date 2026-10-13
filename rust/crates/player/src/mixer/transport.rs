use super::state::{MixerState, PendingTrack};
use crate::MediaSource;
use crate::crossfade::Gain;
use crate::deck::DeckSetting;
use crate::error::{PlayerError, Result, log_failure};

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
        lead.apply(&DeckSetting::Paused(false))
    }

    pub(crate) fn preload_next(&mut self, source: MediaSource<'_>) -> Result<()> {
        self.durations.remember(source.uid, source.duration_ms);
        if !self.config.crossfade_enabled() {
            return self.lead_deck()?.append_next(source.uid, source.url);
        }
        self.pending_next = Some(PendingTrack {
            uid: source.uid,
            url: source.url.to_owned(),
        });
        match self.routing.is_fading() {
            true => Ok(()),
            false => self.preload_pending_into_standby(),
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

    pub(crate) fn set_paused(&self, paused: bool) -> Result<()> {
        let setting = DeckSetting::Paused(paused);
        if self.routing.is_fading() {
            self.apply_to_standby(&setting);
        }
        self.lead_deck()?.apply(&setting)
    }

    pub(crate) fn toggle_pause(&self) -> Result<()> {
        let paused = self.lead_deck()?.is_paused();
        self.set_paused(!paused)
    }

    pub(super) fn preload_pending_into_standby(&mut self) -> Result<()> {
        let Some(pending) = self.pending_next.take() else {
            return Ok(());
        };
        let standby = self.standby_deck().ok_or(PlayerError::Closed)?;
        standby.apply(&DeckSetting::Gain(Gain::SILENT))?;
        standby.apply(&DeckSetting::Paused(true))?;
        standby.replace(pending.uid, &pending.url, 0)?;
        self.durations
            .playing(self.routing.standby_slot(), pending.uid);
        self.routing.preload(pending.uid);
        Ok(())
    }
}
