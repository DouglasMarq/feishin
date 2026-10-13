use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use anyhow::Result;
use feishin_core::queue::Uid;
use feishin_player::{AudioDevice, MediaSource, Player, PlayerConfig, PlayerEvent, ReplayGainMode};
use feishin_storage::settings::{PlaybackSettings, ReplayGainSetting};

pub type EventCallback = Box<dyn Fn(PlayerEvent) + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub uid: Uid,
    pub url: String,
    pub duration_ms: u64,
}

impl Track {
    fn media_source(&self) -> MediaSource<'_> {
        let source = MediaSource::new(self.uid, &self.url);
        match self.duration_ms {
            0 => source,
            duration_ms => source.with_duration_ms(duration_ms),
        }
    }
}

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
    fn set_replay_gain(&self, _mode: ReplayGainSetting) -> Result<()> {
        Ok(())
    }
    fn set_audio_device(&self, _device: Option<&str>) -> Result<()> {
        Ok(())
    }
    fn audio_devices(&self) -> Result<Vec<AudioDevice>> {
        Ok(Vec::new())
    }
    fn set_crossfade(&self, _crossfade: Duration) -> Result<()> {
        Ok(())
    }
}

fn replay_gain_mode(setting: ReplayGainSetting) -> ReplayGainMode {
    match setting {
        ReplayGainSetting::Off => ReplayGainMode::Off,
        ReplayGainSetting::Track => ReplayGainMode::Track,
        ReplayGainSetting::Album => ReplayGainMode::Album,
    }
}

pub struct MpvOutput(Player);

impl MpvOutput {
    pub fn new(settings: &PlaybackSettings, on_event: EventCallback) -> Result<Self> {
        let config = PlayerConfig {
            audio_device: settings.audio_device.clone(),
            replay_gain: replay_gain_mode(settings.replay_gain),
            gapless: settings.gapless,
            volume: settings.volume,
            muted: settings.muted,
            crossfade: settings.crossfade(),
        };
        Ok(Self(Player::new(config, on_event)?))
    }
}

impl AudioOutput for MpvOutput {
    fn load(&self, track: &Track, start_ms: u64) -> Result<()> {
        Ok(self.0.load_source(track.media_source(), start_ms)?)
    }
    fn preload_next(&self, track: &Track) -> Result<()> {
        Ok(self.0.preload_source(track.media_source())?)
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
    fn set_replay_gain(&self, mode: ReplayGainSetting) -> Result<()> {
        Ok(self.0.set_replay_gain(replay_gain_mode(mode))?)
    }
    fn set_audio_device(&self, device: Option<&str>) -> Result<()> {
        Ok(self.0.set_audio_device(device)?)
    }
    fn audio_devices(&self) -> Result<Vec<AudioDevice>> {
        Ok(self.0.audio_devices()?)
    }
    fn set_crossfade(&self, crossfade: Duration) -> Result<()> {
        Ok(self.0.set_crossfade(crossfade)?)
    }
}

#[derive(Default)]
struct FakeState {
    current: Option<Track>,
    next: Option<Track>,
    position_ms: u64,
    playing: bool,
    crossfade_ms: u64,
}

pub struct FakeAudio {
    state: Arc<Mutex<FakeState>>,
    on_event: Arc<EventCallback>,
}

impl FakeAudio {
    pub const TICK: Duration = Duration::from_millis(250);

    pub fn new(on_event: EventCallback) -> Self {
        let state = Arc::new(Mutex::new(FakeState::default()));
        let on_event = Arc::new(on_event);
        let clock_state = Arc::downgrade(&state);
        let clock_events = Arc::clone(&on_event);
        if let Err(error) = std::thread::Builder::new()
            .name("fake-audio".into())
            .spawn(move || Self::clock(clock_state, clock_events))
        {
            tracing::warn!("starting the demo audio clock failed: {error}");
        }
        Self { state, on_event }
    }

    pub fn with_crossfade(self, crossfade: Duration) -> Self {
        if let Err(error) = self.set_crossfade(crossfade) {
            tracing::warn!("setting the demo crossfade failed: {error:#}");
        }
        self
    }

    fn clock(state: Weak<Mutex<FakeState>>, on_event: Arc<EventCallback>) {
        while let Some(shared_state) = state.upgrade() {
            std::thread::sleep(Self::TICK);
            let events = match shared_state.lock() {
                Ok(mut locked_state) => {
                    Self::tick(&mut locked_state, Self::TICK.as_millis() as u64)
                }
                Err(_) => return,
            };
            drop(shared_state);
            events.into_iter().for_each(|event| on_event(event));
        }
    }

    fn tick(state: &mut FakeState, elapsed_ms: u64) -> Vec<PlayerEvent> {
        let Some(duration_ms) = state
            .current
            .as_ref()
            .filter(|_| state.playing)
            .map(|current| current.duration_ms)
        else {
            return Vec::new();
        };
        state.position_ms = (state.position_ms + elapsed_ms).min(duration_ms);
        let mut events = vec![PlayerEvent::Position {
            position_ms: state.position_ms,
            duration_ms: Some(duration_ms),
        }];
        events.extend(Self::advance_when_due(state, duration_ms));
        events
    }

    fn advance_when_due(state: &mut FakeState, duration_ms: u64) -> Vec<PlayerEvent> {
        let overlap_ms = match state.next {
            Some(_) => state.crossfade_ms.min(duration_ms * 2 / 5),
            None => 0,
        };
        if state.position_ms + overlap_ms < duration_ms {
            return Vec::new();
        }
        let ended = state.current.as_ref().filter(|_| overlap_ms == 0);
        let mut events: Vec<PlayerEvent> = ended
            .map(|track| PlayerEvent::TrackEnded { uid: track.uid })
            .into_iter()
            .collect();
        state.position_ms = 0;
        state.current = state.next.take();
        match &state.current {
            Some(next) => events.push(PlayerEvent::Started { uid: next.uid }),
            None => {
                state.playing = false;
                events.push(PlayerEvent::Idle);
            }
        }
        events
    }

    fn update(&self, mutation: impl FnOnce(&mut FakeState) -> Option<PlayerEvent>) -> Result<()> {
        let event = {
            let mut state = self
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("fake audio state poisoned"))?;
            mutation(&mut state)
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
            *state = FakeState {
                crossfade_ms: state.crossfade_ms,
                ..FakeState::default()
            };
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

    fn set_crossfade(&self, crossfade: Duration) -> Result<()> {
        self.update(|state| {
            state.crossfade_ms = u64::try_from(crossfade.as_millis()).unwrap_or(u64::MAX);
            None
        })
    }

    fn audio_devices(&self) -> Result<Vec<AudioDevice>> {
        let device = |name: &str, description: &str| AudioDevice {
            name: name.into(),
            description: description.into(),
        };
        Ok(vec![
            device("auto", "Autoselect device"),
            device("pipewire/demo-speakers", "Demo speakers"),
            device("pipewire/demo-headphones", "Demo headphones"),
        ])
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

    #[test]
    fn fake_clock_crossfades_into_the_preload_before_the_end() {
        let mut state = FakeState {
            current: Some(track(1, 10_000)),
            next: Some(track(2, 10_000)),
            playing: true,
            crossfade_ms: 3_000,
            ..FakeState::default()
        };
        assert_eq!(FakeAudio::tick(&mut state, 6_750).len(), 1);
        let events = FakeAudio::tick(&mut state, 250);
        assert_eq!(events[1..], [PlayerEvent::Started { uid: 2 }]);
        assert_eq!(state.position_ms, 0);
        let last = FakeAudio::tick(&mut state, 10_000);
        assert_eq!(
            last[1..],
            [PlayerEvent::TrackEnded { uid: 2 }, PlayerEvent::Idle]
        );
    }

    #[test]
    fn media_source_carries_the_known_duration() {
        assert_eq!(track(3, 0).media_source().duration_ms, None);
        assert_eq!(track(3, 1_500).media_source().duration_ms, Some(1_500));
    }
}
