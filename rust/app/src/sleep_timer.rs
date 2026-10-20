use std::time::{Duration, Instant};

use slint::ComponentHandle;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::AbortHandle;

use crate::controller::{Command, UiSink, UiUpdate};
use crate::ui::{AppWindow, Context, PlayerState, format_duration, on_ui};

pub const TICK: Duration = Duration::from_secs(1);
const MS_PER_MINUTE: u64 = 60_000;
const MS_PER_SECOND: u64 = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepMode {
    Minutes(u32),
    EndOfTrack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepStatus {
    Remaining { remaining_ms: u64 },
    EndOfTrack,
}

#[derive(Debug, Default)]
pub struct SleepTimer {
    status: Option<SleepStatus>,
    ticker: Option<AbortHandle>,
}

impl SleepTimer {
    pub fn start(&mut self, mode: SleepMode, commands: &UnboundedSender<Command>) {
        self.cancel();
        self.status = Some(match mode {
            SleepMode::Minutes(minutes) => {
                self.ticker = Some(spawn_ticker(commands.clone()));
                SleepStatus::Remaining {
                    remaining_ms: u64::from(minutes) * MS_PER_MINUTE,
                }
            }
            SleepMode::EndOfTrack => SleepStatus::EndOfTrack,
        });
    }

    pub fn cancel(&mut self) {
        if let Some(ticker) = self.ticker.take() {
            ticker.abort();
        }
        self.status = None;
    }

    pub fn status(&self) -> Option<SleepStatus> {
        self.status
    }

    pub fn ends_with_track(&self) -> bool {
        self.status == Some(SleepStatus::EndOfTrack)
    }

    pub fn tick(&mut self, elapsed_ms: u64, playing: bool) -> bool {
        let Some(SleepStatus::Remaining { remaining_ms }) = self.status else {
            return false;
        };
        if !playing {
            return false;
        }
        if elapsed_ms >= remaining_ms {
            self.cancel();
            return true;
        }
        self.status = Some(SleepStatus::Remaining {
            remaining_ms: remaining_ms - elapsed_ms,
        });
        false
    }
}

impl Drop for SleepTimer {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn spawn_ticker(commands: UnboundedSender<Command>) -> AbortHandle {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval_at(tokio::time::Instant::now() + TICK, TICK);
        let mut previous_tick = Instant::now();
        loop {
            interval.tick().await;
            let now = Instant::now();
            let elapsed_ms = now.duration_since(previous_tick).as_millis() as u64;
            previous_tick = now;
            if commands.send(Command::SleepTick { elapsed_ms }).is_err() {
                return;
            }
        }
    })
    .abort_handle()
}

fn sleep_label(status: SleepStatus) -> String {
    match status {
        SleepStatus::Remaining { remaining_ms } => {
            format_duration(remaining_ms.div_ceil(MS_PER_SECOND) * MS_PER_SECOND)
        }
        SleepStatus::EndOfTrack => tr!("End of track"),
    }
}

fn sleep_mode_for_minutes(minutes: i32) -> Option<SleepMode> {
    u32::try_from(minutes)
        .ok()
        .filter(|minutes| *minutes > 0)
        .map(SleepMode::Minutes)
}

pub fn install(window: &AppWindow, context: &Context) {
    let state = window.global::<PlayerState>();
    let start_context = context.clone();
    state.on_start_sleep_timer(move |minutes| {
        if let Some(mode) = sleep_mode_for_minutes(minutes) {
            start_context.send(Command::SleepTimer(Some(mode)));
        }
    });
    let end_of_track_context = context.clone();
    state.on_sleep_at_end_of_track(move || {
        end_of_track_context.send(Command::SleepTimer(Some(SleepMode::EndOfTrack)));
    });
    let cancel_context = context.clone();
    state.on_cancel_sleep_timer(move || cancel_context.send(Command::SleepTimer(None)));
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
            state.set_sleep_active(status.is_some());
            state.set_sleep_label(status.map(sleep_label).unwrap_or_default().into());
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_down_only_while_playing() {
        let mut timer = SleepTimer {
            status: Some(SleepStatus::Remaining {
                remaining_ms: 2_500,
            }),
            ticker: None,
        };
        assert!(!timer.tick(1_000, true));
        assert!(!timer.tick(1_000, false));
        assert_eq!(
            timer.status(),
            Some(SleepStatus::Remaining {
                remaining_ms: 1_500
            })
        );
        assert!(!timer.tick(1_000, true));
        assert!(timer.tick(1_000, true));
        assert_eq!(timer.status(), None);
        assert!(!timer.tick(1_000, true));
    }

    #[test]
    fn end_of_track_ignores_ticks() {
        let mut timer = SleepTimer {
            status: Some(SleepStatus::EndOfTrack),
            ticker: None,
        };
        assert!(!timer.tick(60_000, true));
        assert!(timer.ends_with_track());
    }

    #[test]
    fn given_a_positive_minute_count_the_timer_counts_down_those_minutes() {
        assert_eq!(sleep_mode_for_minutes(15), Some(SleepMode::Minutes(15)));
    }

    #[test]
    fn given_zero_or_negative_minutes_no_timer_starts() {
        assert_eq!(sleep_mode_for_minutes(0), None);
        assert_eq!(sleep_mode_for_minutes(-5), None);
    }

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
