use std::sync::Arc;

use feishin_remote::HostState;
use tokio::sync::watch;

use crate::controller::{UiSink, UiUpdate};
use crate::sleep_timer::SleepStatus;

pub struct HostFeed<Inner> {
    inner: Inner,
    state: watch::Sender<HostState>,
}

impl<Inner> HostFeed<Inner> {
    pub fn new(inner: Inner, state: watch::Sender<HostState>) -> Self {
        Self { inner, state }
    }
}

fn record(state: &mut HostState, update: &UiUpdate) {
    match update {
        UiUpdate::NowPlaying(track) => {
            state.now_playing = track.as_deref().cloned().map(Arc::new);
        }
        UiUpdate::Playback {
            is_playing,
            position_ms,
            duration_ms,
        } => {
            state.is_playing = *is_playing;
            state.position_ms = *position_ms;
            state.duration_ms = *duration_ms;
        }
        UiUpdate::Volume { volume, is_muted } => {
            state.volume = *volume;
            state.is_muted = *is_muted;
        }
        UiUpdate::Modes {
            is_shuffled,
            repeat,
        } => {
            state.is_shuffled = *is_shuffled;
            state.repeat = *repeat;
        }
        UiUpdate::Queue(view) => state.queue = view.clone(),
        UiUpdate::Radio(_) | UiUpdate::Error(_) | UiUpdate::Raise | UiUpdate::Quit => {}
    }
}

fn is_recorded(update: &UiUpdate) -> bool {
    !matches!(
        update,
        UiUpdate::Radio(_) | UiUpdate::Error(_) | UiUpdate::Raise | UiUpdate::Quit
    )
}

impl<Inner: UiSink> UiSink for HostFeed<Inner> {
    fn send(&self, update: UiUpdate) {
        if is_recorded(&update) {
            self.state.send_modify(|state| record(state, &update));
        }
        self.inner.send(update);
    }

    fn sleep_timer(&self, status: Option<SleepStatus>) {
        self.inner.sleep_timer(status);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use feishin_core::queue::RepeatMode;

    use super::*;
    use crate::remote::gate::LocalGate;
    use crate::remote::target::{PlayerTarget, RemoteTarget};

    #[derive(Default)]
    struct Recorded(Mutex<Vec<UiUpdate>>);

    impl UiSink for Arc<Recorded> {
        fn send(&self, update: UiUpdate) {
            self.0.lock().unwrap().push(update);
        }
    }

    #[test]
    fn this_player_keeps_publishing_its_state_while_the_screen_shows_another() {
        let shown = Arc::new(Recorded::default());
        let target = PlayerTarget::default();
        let (state, watched) = watch::channel(HostState::default());
        let sink = HostFeed::new(LocalGate::new(Arc::clone(&shown), target.clone()), state);
        sink.send(UiUpdate::Volume {
            volume: 20,
            is_muted: false,
        });
        let (messages, _sent) = tokio::sync::mpsc::unbounded_channel();
        target.set(Some(RemoteTarget { messages }));
        sink.send(UiUpdate::Modes {
            is_shuffled: true,
            repeat: RepeatMode::One,
        });
        sink.send(UiUpdate::Error("boom".into()));
        assert_eq!(shown.0.lock().unwrap().len(), 2, "volume and the error");
        let host = watched.borrow();
        assert_eq!(
            (host.volume, host.is_shuffled, host.repeat),
            (20, true, RepeatMode::One)
        );
    }
}
