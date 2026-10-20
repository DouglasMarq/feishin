use crate::controller::{UiSink, UiUpdate};
use crate::sleep_timer::SleepStatus;

use super::target::PlayerTarget;

pub struct LocalGate<Inner> {
    inner: Inner,
    target: PlayerTarget,
}

impl<Inner> LocalGate<Inner> {
    pub fn new(inner: Inner, target: PlayerTarget) -> Self {
        Self { inner, target }
    }
}

fn describes_the_player(update: &UiUpdate) -> bool {
    matches!(
        update,
        UiUpdate::NowPlaying(_)
            | UiUpdate::Playback { .. }
            | UiUpdate::Volume { .. }
            | UiUpdate::Modes { .. }
            | UiUpdate::Queue(_)
            | UiUpdate::Radio(_)
    )
}

impl<Inner: UiSink> UiSink for LocalGate<Inner> {
    fn send(&self, update: UiUpdate) {
        if self.target.is_remote() && describes_the_player(&update) {
            return;
        }
        self.inner.send(update);
    }

    fn sleep_timer(&self, status: Option<SleepStatus>) {
        self.inner.sleep_timer(status);
    }
}
