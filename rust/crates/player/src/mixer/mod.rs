mod decks;
mod durations;
mod fade;
mod fader;
mod output;
mod standby;
mod state;
mod transport;

use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, Weak};
use std::time::Duration;

use crate::crossfade::DeckSlot;
use crate::deck::{AudioOutputDriver, Deck};
use crate::error::Result;
use crate::event_loop::EventCallback;
use crate::{PlayerConfig, PlayerEvent};
use decks::Decks;
pub(crate) use fader::spawn_fader;
use state::MixerState;

pub(crate) struct Mixer {
    state: Mutex<MixerState>,
    fader_wake: Condvar,
    on_event: EventCallback,
    this: Weak<Mixer>,
}

impl Mixer {
    pub(crate) fn create(
        config: PlayerConfig,
        driver: AudioOutputDriver,
        on_event: EventCallback,
    ) -> Result<Arc<Self>> {
        let is_crossfade_enabled = config.is_crossfade_enabled();
        let mixer = Arc::new_cyclic(|this| Mixer {
            state: Mutex::new(MixerState::new(config, driver)),
            fader_wake: Condvar::new(),
            on_event,
            this: this.clone(),
        });
        {
            let mut state = mixer.lock_state();
            mixer.ensure_deck(&mut state, DeckSlot::First)?;
            if is_crossfade_enabled {
                mixer.ensure_deck(&mut state, DeckSlot::Second)?;
            }
        }
        Ok(mixer)
    }

    pub(crate) fn update<Outcome>(
        &self,
        change: impl FnOnce(&mut MixerState) -> Outcome,
    ) -> Outcome {
        let (outcome, events) = {
            let mut state = self.lock_state();
            let outcome = change(&mut state);
            self.wake_fader(&mut state);
            (outcome, std::mem::take(&mut state.unreported_events))
        };
        self.emit(events);
        outcome
    }

    pub(crate) fn read<Outcome>(&self, query: impl FnOnce(&MixerState) -> Outcome) -> Outcome {
        query(&self.lock_state())
    }

    pub(crate) fn set_crossfade(&self, crossfade: Duration) -> Result<()> {
        let mut state = self.lock_state();
        if !crossfade.is_zero() {
            self.ensure_deck(&mut state, DeckSlot::Second)?;
        }
        state.change_crossfade(crossfade);
        self.wake_fader(&mut state);
        Ok(())
    }

    pub(crate) fn shut_down(&self) -> Decks {
        let mut state = self.lock_state();
        state.is_shutting_down = true;
        let decks = std::mem::take(&mut state.decks);
        self.wake_fader(&mut state);
        decks
    }

    fn ensure_deck(&self, state: &mut MixerState, slot: DeckSlot) -> Result<()> {
        if state.decks.contains(slot) {
            return Ok(());
        }
        let deck = Deck::create(&state.config, state.driver, self.deck_callback(slot))?;
        state.decks.insert(slot, deck);
        Ok(())
    }

    fn deck_callback(&self, slot: DeckSlot) -> EventCallback {
        let mixer = self.this.clone();
        Box::new(move |event| {
            if let Some(mixer) = mixer.upgrade() {
                mixer.deck_event(slot, event);
            }
        })
    }

    fn deck_event(&self, slot: DeckSlot, event: PlayerEvent) {
        let affects_fader = !matches!(event, PlayerEvent::Position { .. });
        let events = {
            let mut state = self.lock_state();
            let events = state.route(slot, event);
            if affects_fader {
                self.wake_fader(&mut state);
            }
            events
        };
        self.emit(events);
    }

    fn emit(&self, events: Vec<PlayerEvent>) {
        for event in events {
            (self.on_event)(event);
        }
    }

    fn wake_fader(&self, state: &mut MixerState) {
        state.is_fader_woken = true;
        self.fader_wake.notify_one();
    }

    fn lock_state(&self) -> MutexGuard<'_, MixerState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
