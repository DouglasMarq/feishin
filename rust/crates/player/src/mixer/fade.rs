use std::time::Duration;

use super::state::MixerState;
use crate::PlayerEvent;
use crate::crossfade::{
    FADE_STEP, FadeWindow, FaderPlan, Gain, StandbyLoad, plan_before_fade, plan_during_fade,
};
use crate::deck::{Deck, DeckSetting};
use crate::error::log_failure;

pub(super) struct FaderTick {
    pub(super) events: Vec<PlayerEvent>,
    pub(super) wait: Option<Duration>,
}

impl FaderTick {
    fn idle() -> Self {
        Self {
            events: Vec::new(),
            wait: None,
        }
    }

    fn after(delay: Duration) -> Self {
        Self {
            events: Vec::new(),
            wait: Some(delay),
        }
    }
}

impl MixerState {
    pub(super) fn fader_tick(&mut self) -> FaderTick {
        match self.fade {
            Some(window) => self.ramp_fade(window),
            None if !self.config.is_crossfade_enabled() => FaderTick::idle(),
            None if self.routing.standby_is_ready() => self.watch_for_fade_start(),
            None if self.standby_load_is_waiting() => self.watch_for_standby_load(),
            None => FaderTick::idle(),
        }
    }

    pub(crate) fn is_crossfading(&self) -> bool {
        self.routing.is_fading()
    }

    pub(crate) fn holds_next_track(&self) -> bool {
        self.routing.has_standby() && !self.routing.is_fading()
    }

    pub(super) fn finish_fade(&mut self) {
        if !self.routing.is_fading() {
            return;
        }
        self.dismiss_standby();
        self.load_standby_in_background_when_due();
    }

    fn watch_for_standby_load(&mut self) -> FaderTick {
        match self.standby_load_plan() {
            StandbyLoad::Now => {
                self.preload_pending_in_background();
                FaderTick::idle()
            }
            StandbyLoad::Later(wait) => FaderTick::after(wait),
        }
    }

    fn watch_for_fade_start(&mut self) -> FaderTick {
        let Some(lead) = self.lead_clock() else {
            return FaderTick::idle();
        };
        match plan_before_fade(
            self.config.crossfade,
            lead,
            self.incoming_duration_seconds(),
        ) {
            FaderPlan::Begin(window) => self.begin_fade(window),
            FaderPlan::Recheck(delay) => FaderTick::after(delay),
            FaderPlan::Ramp(_) | FaderPlan::Finish => FaderTick::idle(),
        }
    }

    fn begin_fade(&mut self, window: FadeWindow) -> FaderTick {
        let Some(started) = self.routing.begin_fade() else {
            return FaderTick::idle();
        };
        self.fade = Some(window);
        log_failure(
            self.lead_deck()
                .and_then(|incoming| incoming.apply(&DeckSetting::Paused(false))),
            "starting the incoming track",
        );
        FaderTick {
            events: vec![started],
            wait: Some(Duration::ZERO),
        }
    }

    fn ramp_fade(&mut self, window: FadeWindow) -> FaderTick {
        let incoming_position_seconds = self.lead_deck().ok().and_then(Deck::position_seconds);
        match plan_during_fade(window, incoming_position_seconds) {
            FaderPlan::Ramp(gains) => {
                self.apply_gains(gains.outgoing, gains.incoming);
                FaderTick::after(FADE_STEP)
            }
            FaderPlan::Begin(_) | FaderPlan::Recheck(_) | FaderPlan::Finish => {
                self.finish_fade();
                FaderTick::idle()
            }
        }
    }

    fn apply_gains(&self, outgoing: Gain, incoming: Gain) {
        self.apply_to_standby(&DeckSetting::Gain(outgoing));
        log_failure(
            self.lead_deck()
                .and_then(|lead| lead.apply(&DeckSetting::Gain(incoming))),
            "fading in",
        );
    }
}
