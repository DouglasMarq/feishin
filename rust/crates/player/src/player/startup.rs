use std::sync::Arc;

use super::Player;
use crate::deck::AudioOutputDriver;
use crate::error::Result;
use crate::event_loop::EventCallback;
use crate::mixer::{Mixer, spawn_fader};
use crate::{PlayerConfig, PlayerEvent};

impl Player {
    pub fn new(
        config: PlayerConfig,
        on_event: Box<dyn Fn(PlayerEvent) + Send + Sync>,
    ) -> Result<Self> {
        Self::create(config, AudioOutputDriver::System, on_event)
    }

    #[doc(hidden)]
    pub fn with_null_output(
        config: PlayerConfig,
        on_event: Box<dyn Fn(PlayerEvent) + Send + Sync>,
    ) -> Result<Self> {
        Self::create(config, AudioOutputDriver::Null, on_event)
    }

    fn create(
        config: PlayerConfig,
        driver: AudioOutputDriver,
        on_event: EventCallback,
    ) -> Result<Self> {
        let mixer = Mixer::create(config, driver, on_event)?;
        let fader_thread = spawn_fader(Arc::clone(&mixer))?;
        Ok(Self {
            mixer,
            fader_thread: Some(fader_thread),
        })
    }
}
