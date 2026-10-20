mod playlist;
mod setting;
mod startup;

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread::{self, JoinHandle};

use crate::AudioDevice;
use crate::audio_device::audio_devices_from_mpv_list;
use crate::error::{Result, error_from_mpv};
use crate::ffi::{self, Node};
use crate::observed_property::ObservedProperty;
use crate::shared::Shared;
use crate::time::mpv_seconds_from_milliseconds;

pub(crate) use setting::DeckSetting;
pub(crate) use startup::AudioOutputDriver;

pub(crate) const MAX_VOLUME: u8 = 100;
const AUTOMATIC_AUDIO_DEVICE: &str = "auto";

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DeckClock {
    pub(crate) position_seconds: Option<f64>,
    pub(crate) duration_seconds: Option<f64>,
    pub(crate) paused: bool,
}

pub(crate) struct Deck {
    shared: Arc<Shared>,
    event_thread: Option<JoinHandle<()>>,
}

impl Deck {
    pub(crate) fn apply(&self, setting: &DeckSetting) -> Result<()> {
        self.shared
            .mpv
            .set_property(setting.property_name(), setting.mpv_value().as_str())
            .map_err(error_from_mpv)
    }

    pub(crate) fn apply_in_background(&self, setting: &DeckSetting) -> Result<()> {
        ffi::set_property_in_background(
            self.shared.mpv.ctx,
            setting.property_name(),
            &setting.mpv_value(),
        )
    }

    pub(crate) fn is_paused(&self) -> bool {
        self.shared
            .mpv
            .get_property::<bool>(ObservedProperty::Pause.mpv_name())
            .inspect_err(|error| tracing::debug!("reading the mpv pause state failed: {error}"))
            .unwrap_or(false)
    }

    pub(crate) fn seek(&self, position_ms: u64) -> Result<()> {
        let target = mpv_seconds_from_milliseconds(position_ms);
        self.command(&["seek", &target, "absolute"]).map(drop)
    }

    pub(crate) fn audio_devices(&self) -> Result<Vec<AudioDevice>> {
        ffi::get_property_node(self.shared.mpv.ctx, "audio-device-list")
            .map(|list| audio_devices_from_mpv_list(&list))
    }

    pub(crate) fn position_seconds(&self) -> Option<f64> {
        self.shared
            .mpv
            .get_property::<f64>(ObservedProperty::TimePosition.mpv_name())
            .ok()
    }

    pub(crate) fn duration_seconds(&self) -> Option<f64> {
        self.shared
            .mpv
            .get_property::<f64>(ObservedProperty::Duration.mpv_name())
            .ok()
    }

    pub(crate) fn clock(&self) -> DeckClock {
        DeckClock {
            position_seconds: self.position_seconds(),
            duration_seconds: self.duration_seconds(),
            paused: self.is_paused(),
        }
    }

    fn command(&self, arguments: &[&str]) -> Result<Node> {
        ffi::command_with_reply(self.shared.mpv.ctx, arguments)
    }
}

impl Drop for Deck {
    fn drop(&mut self) {
        self.shared.stop_requested.store(true, Ordering::Release);
        ffi::wake_up_event_thread(self.shared.mpv.ctx);
        if let Some(event_thread) = self.event_thread.take()
            && event_thread.thread().id() != thread::current().id()
        {
            event_thread.join().ok();
        }
    }
}
