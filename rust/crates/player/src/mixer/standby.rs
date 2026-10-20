use super::state::{MixerState, PendingTrack};
use crate::crossfade::{Gain, StandbyLoad, plan_standby_load};
use crate::deck::{DeckClock, DeckSetting};
use crate::error::{PlayerError, Result};

impl MixerState {
    pub(super) fn lead_clock(&self) -> Option<DeckClock> {
        let lead = self.lead_deck().ok()?.clock();
        Some(DeckClock {
            duration_seconds: self
                .durations
                .expected_seconds(self.routing.lead(), lead.duration_seconds),
            ..lead
        })
    }

    pub(super) fn incoming_duration_seconds(&self) -> Option<f64> {
        let reported = self
            .standby_deck()
            .and_then(|standby| standby.duration_seconds());
        self.durations
            .expected_seconds(self.routing.standby_slot(), reported)
    }

    pub(super) fn standby_load_is_waiting(&self) -> bool {
        self.pending_next.is_some() && !self.routing.has_standby()
    }

    pub(super) fn standby_load_plan(&self) -> StandbyLoad {
        self.lead_clock().map_or(StandbyLoad::Now, |lead| {
            plan_standby_load(self.config.crossfade, lead)
        })
    }

    pub(super) fn load_standby_when_due(&mut self) -> Result<()> {
        if self.pending_next.is_none() {
            return Ok(());
        }
        match self.standby_load_plan() {
            StandbyLoad::Now => self.preload_pending_into_standby(),
            StandbyLoad::Later(_) => {
                self.dismiss_standby();
                Ok(())
            }
        }
    }

    pub(super) fn load_standby_in_background_when_due(&mut self) {
        if self.pending_next.is_some() && self.standby_load_plan() == StandbyLoad::Now {
            self.preload_pending_in_background();
        }
    }

    pub(super) fn preload_pending_in_background(&mut self) {
        let pending_uid = self.pending_next.as_ref().map(|pending| pending.uid);
        if let (Err(error), Some(uid)) = (self.preload_pending_into_standby(), pending_uid) {
            tracing::warn!("preloading the next track failed: {error}");
            self.routing.preload_failed(uid, error.to_string());
        }
    }

    fn preload_pending_into_standby(&mut self) -> Result<()> {
        let Some(pending) = self.pending_next.take() else {
            return Ok(());
        };
        self.durations
            .playing(self.routing.standby_slot(), pending.uid);
        self.routing.preload(pending.uid);
        self.start_standby(&pending)
            .inspect_err(|_| self.routing.clear_standby())
    }

    fn start_standby(&self, pending: &PendingTrack) -> Result<()> {
        let standby = self.standby_deck().ok_or(PlayerError::Closed)?;
        standby.apply(&DeckSetting::Gain(Gain::SILENT))?;
        standby.apply(&DeckSetting::Paused(true))?;
        standby.replace(pending.uid, &pending.url, 0)
    }
}
