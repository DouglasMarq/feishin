//! Audio playback through libmpv.
//!
//! Events are delivered on a dedicated mpv event thread through the callback passed to `Player::new`;
//! the app forwards them to the UI thread. `Player` methods can be called from any thread, including
//! from inside the callback.

mod event_loop;
mod ffi;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};

use feishin_core::queue::Uid;
use libmpv2::{Format, Mpv, SetData};

use crate::ffi::Node;

#[derive(Debug, thiserror::Error)]
pub enum PlayerError {
    #[error("mpv error: {0}")]
    Mpv(String),
}

pub type Result<T> = std::result::Result<T, PlayerError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReplayGainMode {
    #[default]
    Off,
    Track,
    Album,
}

impl ReplayGainMode {
    fn mpv_value(self) -> &'static str {
        match self {
            ReplayGainMode::Off => "no",
            ReplayGainMode::Track => "track",
            ReplayGainMode::Album => "album",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerConfig {
    /// mpv `audio-device` name; `None` uses the default output.
    pub audio_device: Option<String>,
    pub replay_gain: ReplayGainMode,
    pub gapless: bool,
    /// 0-100.
    pub volume: u8,
}

impl Default for PlayerConfig {
    fn default() -> Self {
        Self {
            audio_device: None,
            replay_gain: ReplayGainMode::Off,
            gapless: true,
            volume: 70,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerEvent {
    /// A track began playing, either from `load` or because a preloaded track took over (gapless).
    Started {
        uid: Uid,
    },
    /// Emitted about every 250 ms while playing.
    Position {
        position_ms: u64,
        duration_ms: Option<u64>,
    },
    Paused,
    Resumed,
    /// The track played to its end (not emitted for `stop` or `load` replacing it).
    TrackEnded {
        uid: Uid,
    },
    Error {
        uid: Option<Uid>,
        message: String,
    },
    /// Nothing left to play.
    Idle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDevice {
    pub name: String,
    pub description: String,
}

#[derive(Default)]
struct State {
    uids: HashMap<i64, Uid>,
    next_entry: Option<i64>,
}

struct Shared {
    mpv: Mpv,
    stop: AtomicBool,
    state: Mutex<State>,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub struct Player {
    shared: Arc<Shared>,
    events: Option<JoinHandle<()>>,
}

/// No `reconnect_at_eof`: FLAC seeking probes the end of the stream, and reconnecting on every
/// probe made starting mid-track take 10-30 s instead of under 1 s.
const STREAM_LAVF_OPTIONS: &str = "reconnect=1,reconnect_streamed=1,reconnect_delay_max=5";

impl Player {
    pub fn new(
        config: PlayerConfig,
        on_event: Box<dyn Fn(PlayerEvent) + Send + Sync>,
    ) -> Result<Self> {
        Self::create(config, None, on_event)
    }

    /// Like `new`, but with mpv's `ao=null` output so nothing reaches the sound card. Meant for tests.
    #[doc(hidden)]
    pub fn with_null_output(
        config: PlayerConfig,
        on_event: Box<dyn Fn(PlayerEvent) + Send + Sync>,
    ) -> Result<Self> {
        Self::create(config, Some("null"), on_event)
    }

    fn create(
        config: PlayerConfig,
        ao: Option<&str>,
        on_event: Box<dyn Fn(PlayerEvent) + Send + Sync>,
    ) -> Result<Self> {
        let flag = |on: bool| if on { "yes" } else { "no" };
        let volume = config.volume.min(100).to_string();
        let mut options: Vec<(&str, &str)> = vec![
            ("vo", "null"),
            ("vid", "no"),
            ("audio-display", "no"),
            ("idle", "yes"),
            ("load-scripts", "no"),
            ("ytdl", "no"),
            ("gapless-audio", if config.gapless { "weak" } else { "no" }),
            ("prefetch-playlist", flag(config.gapless)),
            ("network-timeout", "10"),
            ("stream-lavf-o", STREAM_LAVF_OPTIONS),
            ("replaygain", config.replay_gain.mpv_value()),
            ("replaygain-clip", "no"),
            ("volume", &volume),
            (
                "audio-device",
                config.audio_device.as_deref().unwrap_or("auto"),
            ),
        ];
        if let Some(ao) = ao {
            options.push(("ao", ao));
        }

        // SAFETY: libmpv refuses to initialize unless LC_NUMERIC is "C"; the argument is a valid C string.
        unsafe { libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr()) };

        let mpv = Mpv::with_initializer(|init| {
            for (name, value) in &options {
                init.set_property(name, *value)?;
            }
            for name in ["osc", "input-default-bindings", "input-vo-keyboard"] {
                init.set_property(name, "no").ok();
            }
            Ok(())
        })
        .map_err(mpv_error)?;

        mpv.observe_property("pause", Format::Flag, event_loop::PROP_PAUSE)
            .map_err(mpv_error)?;
        mpv.observe_property("time-pos", Format::Double, event_loop::PROP_TIME_POS)
            .map_err(mpv_error)?;
        mpv.observe_property("duration", Format::Double, event_loop::PROP_DURATION)
            .map_err(mpv_error)?;
        mpv.observe_property("idle-active", Format::Flag, event_loop::PROP_IDLE)
            .map_err(mpv_error)?;

        let shared = Arc::new(Shared {
            mpv,
            stop: AtomicBool::new(false),
            state: Mutex::new(State::default()),
        });
        let thread_shared = Arc::clone(&shared);
        let events = thread::Builder::new()
            .name("mpv-events".into())
            .spawn(move || event_loop::run(thread_shared, on_event))
            .map_err(|e| PlayerError::Mpv(format!("failed to spawn event thread: {e}")))?;

        Ok(Self {
            shared,
            events: Some(events),
        })
    }

    /// Replace whatever is playing with `url`, start at `start_ms` and unpause. Clears any preloaded track.
    pub fn load(&self, uid: Uid, url: &str, start_ms: u64) -> Result<()> {
        let mut state = self.shared.lock();
        let start =
            (start_ms > 0).then(|| format!("start={}.{:03}", start_ms / 1000, start_ms % 1000));
        let entry = self.loadfile(url, "replace", start.as_deref())?;
        state.uids.clear();
        state.uids.insert(entry, uid);
        state.next_entry = None;
        drop(state);
        self.set("pause", false)
    }

    /// Queue `url` to start right after the current track (gapless when enabled). Replaces a previous preload.
    pub fn preload_next(&self, uid: Uid, url: &str) -> Result<()> {
        let mut state = self.shared.lock();
        self.clear_next_locked(&mut state)?;
        let entry = self.loadfile(url, "append", None)?;
        state.uids.insert(entry, uid);
        state.next_entry = Some(entry);
        Ok(())
    }

    pub fn clear_next(&self) -> Result<()> {
        let mut state = self.shared.lock();
        self.clear_next_locked(&mut state)
    }

    pub fn play(&self) -> Result<()> {
        self.set("pause", false)
    }

    pub fn pause(&self) -> Result<()> {
        self.set("pause", true)
    }

    pub fn toggle_pause(&self) -> Result<()> {
        self.command(&["cycle", "pause"]).map(drop)
    }

    pub fn stop(&self) -> Result<()> {
        let mut state = self.shared.lock();
        self.command(&["stop"])?;
        state.uids.clear();
        state.next_entry = None;
        Ok(())
    }

    pub fn seek(&self, position_ms: u64) -> Result<()> {
        let target = format!("{}.{:03}", position_ms / 1000, position_ms % 1000);
        self.command(&["seek", &target, "absolute"]).map(drop)
    }

    pub fn set_volume(&self, volume: u8) -> Result<()> {
        self.set("volume", i64::from(volume.min(100)))
    }

    pub fn set_muted(&self, muted: bool) -> Result<()> {
        self.set("mute", muted)
    }

    pub fn set_replay_gain(&self, mode: ReplayGainMode) -> Result<()> {
        self.set("replaygain", mode.mpv_value())
    }

    pub fn set_audio_device(&self, device: Option<&str>) -> Result<()> {
        self.set("audio-device", device.unwrap_or("auto"))
    }

    pub fn audio_devices(&self) -> Result<Vec<AudioDevice>> {
        let list = ffi::get_node(self.shared.mpv.ctx, "audio-device-list").map_err(code_error)?;
        let Node::Array(entries) = list else {
            return Ok(Vec::new());
        };
        Ok(entries
            .iter()
            .map(|entry| {
                let field = |key| {
                    entry
                        .get(key)
                        .and_then(Node::as_str)
                        .filter(|s| !s.is_empty())
                };
                let description = field("description");
                AudioDevice {
                    name: field("name")
                        .or(description)
                        .unwrap_or("Unknown Device")
                        .to_owned(),
                    description: description.unwrap_or_default().to_owned(),
                }
            })
            .collect())
    }

    pub fn position_ms(&self) -> Option<u64> {
        self.shared
            .mpv
            .get_property::<f64>("time-pos")
            .ok()
            .map(event_loop::secs_to_ms)
    }

    fn clear_next_locked(&self, state: &mut State) -> Result<()> {
        self.command(&["playlist-clear"])?;
        if let Some(old) = state.next_entry.take()
            && !self.playlist_contains(old)
        {
            state.uids.remove(&old);
        }
        Ok(())
    }

    fn playlist_contains(&self, entry: i64) -> bool {
        match ffi::get_node(self.shared.mpv.ctx, "playlist") {
            Ok(Node::Array(items)) => items
                .iter()
                .any(|item| item.get("id").and_then(Node::as_i64) == Some(entry)),
            _ => false,
        }
    }

    fn loadfile(&self, url: &str, mode: &str, options: Option<&str>) -> Result<i64> {
        let reply = match options {
            None => self.command(&["loadfile", url, mode])?,
            Some(options) => self
                .command(&["loadfile", url, mode, "-1", options])
                .or_else(|_| self.command(&["loadfile", url, mode, options]))?,
        };
        reply
            .get("playlist_entry_id")
            .and_then(Node::as_i64)
            .ok_or_else(|| PlayerError::Mpv("loadfile returned no playlist entry id".into()))
    }

    fn command(&self, args: &[&str]) -> Result<Node> {
        ffi::command_ret(self.shared.mpv.ctx, args).map_err(code_error)
    }

    fn set<T: SetData>(&self, name: &str, value: T) -> Result<()> {
        self.shared.mpv.set_property(name, value).map_err(mpv_error)
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        ffi::wakeup(self.shared.mpv.ctx);
        if let Some(events) = self.events.take()
            && events.thread().id() != thread::current().id()
        {
            events.join().ok();
        }
    }
}

fn code_error(code: std::ffi::c_int) -> PlayerError {
    PlayerError::Mpv(ffi::error_string(code))
}

fn mpv_error(err: libmpv2::Error) -> PlayerError {
    match err {
        libmpv2::Error::Raw(code) => code_error(code),
        other => PlayerError::Mpv(other.to_string()),
    }
}
