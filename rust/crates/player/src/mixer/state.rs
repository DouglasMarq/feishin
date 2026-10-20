use std::time::Duration;

use feishin_core::queue::Uid;

use super::decks::Decks;
use super::durations::TrackDurations;
use crate::crossfade::{DeckSlot, FadeWindow, Gain, Routing, ends_track};
use crate::deck::{AudioOutputDriver, Deck, DeckSetting};
use crate::error::{PlayerError, Result, log_failure};
use crate::{PlayerConfig, PlayerEvent};

pub(crate) struct PendingTrack {
    pub(super) uid: Uid,
    pub(super) url: String,
}

pub(crate) struct MixerState {
    pub(super) decks: Decks,
    pub(super) routing: Routing,
    pub(super) fade: Option<FadeWindow>,
    pub(super) config: PlayerConfig,
    pub(super) driver: AudioOutputDriver,
    pub(super) pending_next: Option<PendingTrack>,
    pub(super) durations: TrackDurations,
    pub(super) unreported_events: Vec<PlayerEvent>,
    pub(super) fader_woken: bool,
    pub(super) shutting_down: bool,
}

impl MixerState {
    pub(super) fn new(config: PlayerConfig, driver: AudioOutputDriver) -> Self {
        Self {
            decks: Decks::default(),
            routing: Routing::new(),
            fade: None,
            config,
            driver,
            pending_next: None,
            durations: TrackDurations::default(),
            unreported_events: Vec::new(),
            fader_woken: false,
            shutting_down: false,
        }
    }

    pub(crate) fn lead_deck(&self) -> Result<&Deck> {
        self.decks
            .get(self.routing.lead())
            .ok_or(PlayerError::Closed)
    }

    pub(super) fn standby_deck(&self) -> Option<&Deck> {
        self.decks.get(self.routing.standby_slot())
    }

    pub(super) fn set_lead_paused(&mut self, paused: bool) -> Result<()> {
        self.lead_deck()?.apply(&DeckSetting::Paused(paused))?;
        let reported = self.routing.pause_changed(paused).events;
        self.unreported_events.extend(reported);
        Ok(())
    }

    pub(super) fn apply_to_standby(&self, setting: &DeckSetting) {
        if let Some(standby) = self.standby_deck() {
            log_failure(
                standby.apply_in_background(setting),
                "updating the standby deck",
            );
        }
    }

    pub(super) fn route(&mut self, slot: DeckSlot, event: PlayerEvent) -> Vec<PlayerEvent> {
        let event = self.with_current_pause_state(slot, event);
        if let PlayerEvent::Started { uid } = event {
            self.durations.playing(slot, uid);
        }
        let lead_ended = slot == self.routing.lead() && ends_track(&event);
        let outgoing_ended = self.routing.outgoing_deck_went_idle(slot, &event);
        if lead_ended {
            self.finish_fade();
            self.preload_pending_in_background();
        }
        let outcome = self.routing.route(slot, event);
        if outgoing_ended {
            self.finish_fade();
        }
        if outcome.promoted {
            self.start_promoted_lead();
        }
        outcome.events
    }

    fn with_current_pause_state(&self, slot: DeckSlot, event: PlayerEvent) -> PlayerEvent {
        let reports_pause = matches!(event, PlayerEvent::Paused | PlayerEvent::Resumed);
        match self.lead_deck() {
            Ok(lead) if reports_pause && slot == self.routing.lead() => match lead.is_paused() {
                true => PlayerEvent::Paused,
                false => PlayerEvent::Resumed,
            },
            _ => event,
        }
    }

    pub(super) fn change_crossfade(&mut self, crossfade: Duration) {
        let toggled = self.config.crossfade.is_zero() != crossfade.is_zero();
        self.config.crossfade = crossfade;
        if toggled {
            self.pending_next = None;
            if !self.routing.is_fading() {
                self.dismiss_standby();
            }
            log_failure(
                self.lead_deck().and_then(Deck::clear_next),
                "clearing the next track",
            );
        }
    }

    pub(super) fn dismiss_standby(&mut self) {
        if !self.routing.has_standby() {
            return;
        }
        let was_fading = self.routing.is_fading();
        self.routing.clear_standby();
        self.fade = None;
        if let Some(standby) = self.standby_deck() {
            log_failure(standby.stop(), "stopping the standby deck");
        }
        if was_fading {
            log_failure(
                self.lead_deck()
                    .and_then(|lead| lead.apply(&DeckSetting::Gain(Gain::UNITY))),
                "restoring the gain after a crossfade",
            );
        }
    }

    fn start_promoted_lead(&self) {
        let Ok(lead) = self.lead_deck() else {
            return;
        };
        log_failure(
            lead.apply(&DeckSetting::Gain(Gain::UNITY)),
            "restoring the gain",
        );
        log_failure(
            lead.apply(&DeckSetting::Paused(false)),
            "starting the next track",
        );
    }
}
