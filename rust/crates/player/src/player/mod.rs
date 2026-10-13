mod playlist;
mod startup;

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread::{self, JoinHandle};

use libmpv2::SetData;

use crate::audio_device::audio_devices_from_mpv_list;
use crate::error::{Result, error_from_mpv, error_from_mpv_code};
use crate::ffi::{self, Node};
use crate::shared::Shared;
use crate::time::{mpv_seconds_from_milliseconds, seconds_to_milliseconds};
use crate::{AudioDevice, ReplayGainMode};

const MAX_VOLUME: u8 = 100;
const AUTOMATIC_AUDIO_DEVICE: &str = "auto";

pub struct Player {
    shared: Arc<Shared>,
    event_thread: Option<JoinHandle<()>>,
}

impl Player {
    pub fn play(&self) -> Result<()> {
        self.set("pause", false)
    }

    pub fn pause(&self) -> Result<()> {
        self.set("pause", true)
    }

    pub fn toggle_pause(&self) -> Result<()> {
        self.command(&["cycle", "pause"]).map(drop)
    }

    pub fn seek(&self, position_ms: u64) -> Result<()> {
        let target = mpv_seconds_from_milliseconds(position_ms);
        self.command(&["seek", &target, "absolute"]).map(drop)
    }

    pub fn set_volume(&self, volume: u8) -> Result<()> {
        self.set("volume", i64::from(volume.min(MAX_VOLUME)))
    }

    pub fn set_muted(&self, muted: bool) -> Result<()> {
        self.set("mute", muted)
    }

    pub fn set_replay_gain(&self, mode: ReplayGainMode) -> Result<()> {
        self.set("replaygain", mode.mpv_value())
    }

    pub fn set_audio_device(&self, device: Option<&str>) -> Result<()> {
        self.set("audio-device", device.unwrap_or(AUTOMATIC_AUDIO_DEVICE))
    }

    pub fn audio_devices(&self) -> Result<Vec<AudioDevice>> {
        ffi::get_property_node(self.shared.mpv.ctx, "audio-device-list")
            .map(|list| audio_devices_from_mpv_list(&list))
            .map_err(error_from_mpv_code)
    }

    pub fn position_ms(&self) -> Option<u64> {
        self.shared
            .mpv
            .get_property::<f64>("time-pos")
            .ok()
            .map(seconds_to_milliseconds)
    }

    fn command(&self, arguments: &[&str]) -> Result<Node> {
        ffi::command_with_reply(self.shared.mpv.ctx, arguments).map_err(error_from_mpv_code)
    }

    fn set<Value: SetData>(&self, name: &str, value: Value) -> Result<()> {
        self.shared
            .mpv
            .set_property(name, value)
            .map_err(error_from_mpv)
    }
}

impl Drop for Player {
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
