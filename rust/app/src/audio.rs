use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use anyhow::Result;
use feishin_core::queue::Uid;
use feishin_player::{Player, PlayerConfig, PlayerEvent, ReplayGainMode};
use feishin_storage::settings::{PlaybackSettings, ReplayGainSetting};

pub type EventCallback = Box<dyn Fn(PlayerEvent) + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub uid: Uid,
    pub url: String,
    pub duration_ms: u64,
}

/// What the controller needs from an audio backend (`feishin_player::Player` in real mode).
pub trait AudioOutput: Send {
    fn load(&self, track: &Track, start_ms: u64) -> Result<()>;
    fn preload_next(&self, track: &Track) -> Result<()>;
    fn clear_next(&self) -> Result<()>;
    fn play(&self) -> Result<()>;
    fn pause(&self) -> Result<()>;
    fn stop(&self) -> Result<()>;
    fn seek(&self, position_ms: u64) -> Result<()>;
    fn set_volume(&self, volume: u8) -> Result<()>;
    fn set_muted(&self, muted: bool) -> Result<()>;
}

pub struct MpvOutput(Player);

impl MpvOutput {
    pub fn new(settings: &PlaybackSettings, on_event: EventCallback) -> Result<Self> {
        let config = PlayerConfig {
            audio_device: settings.audio_device.clone(),
            replay_gain: match settings.replay_gain {
                ReplayGainSetting::Off => ReplayGainMode::Off,
                ReplayGainSetting::Track => ReplayGainMode::Track,
                ReplayGainSetting::Album => ReplayGainMode::Album,
            },
            gapless: settings.gapless,
            volume: settings.volume,
        };
        let player = Player::new(config, on_event)?;
        player.set_muted(settings.muted)?;
        Ok(Self(player))
    }
}

impl AudioOutput for MpvOutput {
    fn load(&self, track: &Track, start_ms: u64) -> Result<()> {
        Ok(self.0.load(track.uid, &track.url, start_ms)?)
    }
    fn preload_next(&self, track: &Track) -> Result<()> {
        Ok(self.0.preload_next(track.uid, &track.url)?)
    }
    fn clear_next(&self) -> Result<()> {
        Ok(self.0.clear_next()?)
    }
    fn play(&self) -> Result<()> {
        Ok(self.0.play()?)
    }
    fn pause(&self) -> Result<()> {
        Ok(self.0.pause()?)
    }
    fn stop(&self) -> Result<()> {
        Ok(self.0.stop()?)
    }
    fn seek(&self, position_ms: u64) -> Result<()> {
        Ok(self.0.seek(position_ms)?)
    }
    fn set_volume(&self, volume: u8) -> Result<()> {
        Ok(self.0.set_volume(volume)?)
    }
    fn set_muted(&self, muted: bool) -> Result<()> {
        Ok(self.0.set_muted(muted)?)
    }
}

#[derive(Default)]
struct FakeState {
    current: Option<Track>,
    next: Option<Track>,
    position_ms: u64,
    playing: bool,
}

/// Silent output for `--demo`: advances a clock and emits the same events as the real player.
pub struct FakeAudio {
    state: Arc<Mutex<FakeState>>,
    on_event: Arc<EventCallback>,
}

impl FakeAudio {
    pub const TICK: Duration = Duration::from_millis(250);

    pub fn new(on_event: EventCallback) -> Self {
        let state = Arc::new(Mutex::new(FakeState::default()));
        let on_event = Arc::new(on_event);
        let weak = Arc::downgrade(&state);
        let events = Arc::clone(&on_event);
        std::thread::Builder::new()
            .name("fake-audio".into())
            .spawn(move || Self::clock(weak, events))
            .ok();
        Self { state, on_event }
    }

    fn clock(state: Weak<Mutex<FakeState>>, on_event: Arc<EventCallback>) {
        while let Some(state) = state.upgrade() {
            std::thread::sleep(Self::TICK);
            let events = match state.lock() {
                Ok(mut state) => Self::tick(&mut state, Self::TICK.as_millis() as u64),
                Err(_) => return,
            };
            drop(state);
            events.into_iter().for_each(|event| on_event(event));
        }
    }

    fn tick(state: &mut FakeState, elapsed_ms: u64) -> Vec<PlayerEvent> {
        let Some(current) = state.current.as_ref().filter(|_| state.playing) else {
            return Vec::new();
        };
        let duration_ms = current.duration_ms;
        let uid = current.uid;
        state.position_ms = (state.position_ms + elapsed_ms).min(duration_ms);
        let mut events = vec![PlayerEvent::Position {
            position_ms: state.position_ms,
            duration_ms: Some(duration_ms),
        }];
        if state.position_ms >= duration_ms {
            events.push(PlayerEvent::TrackEnded { uid });
            state.position_ms = 0;
            state.current = state.next.take();
            match &state.current {
                Some(next) => events.push(PlayerEvent::Started { uid: next.uid }),
                None => {
                    state.playing = false;
                    events.push(PlayerEvent::Idle);
                }
            }
        }
        events
    }

    fn update(&self, f: impl FnOnce(&mut FakeState) -> Option<PlayerEvent>) -> Result<()> {
        let event = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("fake audio state poisoned"))?;
            f(&mut state)
        };
        if let Some(event) = event {
            (self.on_event)(event);
        }
        Ok(())
    }
}

impl AudioOutput for FakeAudio {
    fn load(&self, track: &Track, start_ms: u64) -> Result<()> {
        self.update(|state| {
            state.current = Some(track.clone());
            state.next = None;
            state.position_ms = start_ms;
            state.playing = true;
            Some(PlayerEvent::Started { uid: track.uid })
        })
    }

    fn preload_next(&self, track: &Track) -> Result<()> {
        self.update(|state| {
            state.next = Some(track.clone());
            None
        })
    }

    fn clear_next(&self) -> Result<()> {
        self.update(|state| {
            state.next = None;
            None
        })
    }

    fn play(&self) -> Result<()> {
        self.update(|state| {
            let resumed = state.current.is_some() && !state.playing;
            state.playing |= state.current.is_some();
            resumed.then_some(PlayerEvent::Resumed)
        })
    }

    fn pause(&self) -> Result<()> {
        self.update(|state| {
            let paused = state.playing;
            state.playing = false;
            paused.then_some(PlayerEvent::Paused)
        })
    }

    fn stop(&self) -> Result<()> {
        self.update(|state| {
            *state = FakeState::default();
            None
        })
    }

    fn seek(&self, position_ms: u64) -> Result<()> {
        self.update(|state| {
            let duration_ms = state.current.as_ref()?.duration_ms;
            state.position_ms = position_ms.min(duration_ms);
            Some(PlayerEvent::Position {
                position_ms: state.position_ms,
                duration_ms: Some(duration_ms),
            })
        })
    }

    fn set_volume(&self, _volume: u8) -> Result<()> {
        Ok(())
    }

    fn set_muted(&self, _muted: bool) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(uid: Uid, duration_ms: u64) -> Track {
        Track {
            uid,
            url: String::new(),
            duration_ms,
        }
    }

    #[test]
    fn fake_clock_ends_track_and_takes_over_preload() {
        let mut state = FakeState {
            current: Some(track(1, 500)),
            next: Some(track(2, 500)),
            playing: true,
            ..FakeState::default()
        };
        assert_eq!(FakeAudio::tick(&mut state, 250).len(), 1);
        let events = FakeAudio::tick(&mut state, 250);
        assert_eq!(events[1], PlayerEvent::TrackEnded { uid: 1 });
        assert_eq!(events[2], PlayerEvent::Started { uid: 2 });
        FakeAudio::tick(&mut state, 500);
        assert_eq!(FakeAudio::tick(&mut state, 0), Vec::new());
    }
}
