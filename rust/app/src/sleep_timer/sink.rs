use slint::ComponentHandle;

use super::SleepStatus;
use crate::controller::{UiSink, UiUpdate};
use crate::ui::{AppWindow, PlayerState, format_duration, on_ui};

const MS_PER_SECOND: u64 = 1000;

fn sleep_label(status: SleepStatus) -> String {
    match status {
        SleepStatus::Remaining { remaining_ms } => {
            format_duration(remaining_ms.div_ceil(MS_PER_SECOND) * MS_PER_SECOND)
        }
        SleepStatus::EndOfTrack => tr!("End of track"),
    }
}

pub struct SleepTimerUi<Inner> {
    inner: Inner,
    window: slint::Weak<AppWindow>,
}

impl<Inner> SleepTimerUi<Inner> {
    pub fn new(inner: Inner, window: slint::Weak<AppWindow>) -> Self {
        Self { inner, window }
    }
}

impl<Inner: UiSink> UiSink for SleepTimerUi<Inner> {
    fn send(&self, update: UiUpdate) {
        self.inner.send(update);
    }

    fn sleep_timer(&self, status: Option<SleepStatus>) {
        on_ui(&self.window, move |window| {
            let state = window.global::<PlayerState>();
            state.set_is_sleep_active(status.is_some());
            state.set_sleep_label(status.map(sleep_label).unwrap_or_default().into());
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_round_up_to_the_second() {
        assert_eq!(
            sleep_label(SleepStatus::Remaining {
                remaining_ms: 61_200
            }),
            "1:02"
        );
        assert_eq!(
            sleep_label(SleepStatus::Remaining {
                remaining_ms: 15 * 60_000
            }),
            "15:00"
        );
        assert_eq!(sleep_label(SleepStatus::EndOfTrack), "End of track");
    }
}
