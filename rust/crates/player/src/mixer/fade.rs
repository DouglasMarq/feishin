use std::time::Duration;

use super::state::MixerState;
use crate::PlayerEvent;
use crate::crossfade::{
    FADE_STEP, FadeWindow, FaderPlan, Gain, plan_before_fade, plan_during_fade,
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
            None if self.config.crossfade_enabled() && self.routing.standby_is_ready() => {
                self.watch_for_fade_start()
            }
            None => FaderTick::idle(),
        }
    }

    pub(crate) fn is_crossfading(&self) -> bool {
        self.routing.is_fading()
    }

    pub(super) fn finish_fade(&mut self) {
        if !self.routing.is_fading() {
            return;
        }
        self.dismiss_standby();
        log_failure(
            self.preload_pending_into_standby(),
            "preloading the next track",
        );
    }

    fn watch_for_fade_start(&mut self) -> FaderTick {
        let Ok(lead) = self.lead_deck() else {
            return FaderTick::idle();
        };
        let mut clock = lead.clock();
        clock.duration = self
            .durations
            .expected_seconds(self.routing.lead())
            .or(clock.duration);
        match plan_before_fade(self.config.crossfade, clock) {
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
        let incoming_position = self.lead_deck().ok().and_then(Deck::position_seconds);
        match plan_during_fade(window, incoming_position) {
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
