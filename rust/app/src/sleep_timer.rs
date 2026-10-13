//! Sleep timer: pauses playback after a delay of listening time, or when the current track ends.

use std::time::{Duration, Instant};

use slint::ComponentHandle;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::AbortHandle;

use crate::controller::{Command, UiSink, UiUpdate};
use crate::ui::{AppWindow, Context, PlayerState, format_duration, on_ui};

pub const TICK: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepMode {
    Minutes(u32),
    EndOfTrack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepStatus {
    Remaining { ms: u64 },
    EndOfTrack,
}

#[derive(Debug, Default)]
pub struct SleepTimer {
    status: Option<SleepStatus>,
    ticker: Option<AbortHandle>,
}

impl SleepTimer {
    /// Starts (or replaces) the timer. Timed modes tick through `commands`, so this needs a Tokio runtime.
    pub fn start(&mut self, mode: SleepMode, commands: &UnboundedSender<Command>) {
        self.cancel();
        self.status = Some(match mode {
            SleepMode::Minutes(minutes) => {
                self.ticker = Some(spawn_ticker(commands.clone()));
                SleepStatus::Remaining {
                    ms: u64::from(minutes) * 60_000,
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

    /// Counts down listening time only; returns `true` when the timer fires (it is then cancelled).
    pub fn tick(&mut self, elapsed_ms: u64, playing: bool) -> bool {
        let Some(SleepStatus::Remaining { ms }) = self.status else {
            return false;
        };
        if !playing {
            return false;
        }
        if elapsed_ms >= ms {
            self.cancel();
            return true;
        }
        self.status = Some(SleepStatus::Remaining {
            ms: ms - elapsed_ms,
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
        let mut last = Instant::now();
        loop {
            interval.tick().await;
            let now = Instant::now();
            let elapsed_ms = now.duration_since(last).as_millis() as u64;
            last = now;
            if commands.send(Command::SleepTick { elapsed_ms }).is_err() {
                return;
            }
        }
    })
    .abort_handle()
}

fn label(status: SleepStatus) -> String {
    match status {
        SleepStatus::Remaining { ms } => format_duration(ms.div_ceil(1000) * 1000),
        SleepStatus::EndOfTrack => "End of track".into(),
    }
}

pub fn install(window: &AppWindow, ctx: &Context) {
    let state = window.global::<PlayerState>();
    let start_ctx = ctx.clone();
    state.on_start_sleep_timer(move |minutes| {
        let mode = match u32::try_from(minutes) {
            Ok(0) | Err(_) => SleepMode::EndOfTrack,
            Ok(minutes) => SleepMode::Minutes(minutes),
        };
        start_ctx.send(Command::SleepTimer(Some(mode)));
    });
    let cancel_ctx = ctx.clone();
    state.on_cancel_sleep_timer(move || cancel_ctx.send(Command::SleepTimer(None)));
}

/// Forwards controller updates to `inner` and shows the sleep timer in the player bar.
pub struct SleepTimerUi<S> {
    inner: S,
    window: slint::Weak<AppWindow>,
}

impl<S> SleepTimerUi<S> {
    pub fn new(inner: S, window: slint::Weak<AppWindow>) -> Self {
        Self { inner, window }
    }
}

impl<S: UiSink> UiSink for SleepTimerUi<S> {
    fn send(&self, update: UiUpdate) {
        self.inner.send(update);
    }

    fn sleep_timer(&self, status: Option<SleepStatus>) {
        on_ui(&self.window, move |window| {
            let state = window.global::<PlayerState>();
            state.set_sleep_active(status.is_some());
            state.set_sleep_label(status.map(label).unwrap_or_default().into());
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_down_only_while_playing() {
        let mut timer = SleepTimer {
            status: Some(SleepStatus::Remaining { ms: 2_500 }),
            ticker: None,
        };
        assert!(!timer.tick(1_000, true));
        assert!(!timer.tick(1_000, false));
        assert_eq!(timer.status(), Some(SleepStatus::Remaining { ms: 1_500 }));
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
    fn labels_round_up_to_the_second() {
        assert_eq!(label(SleepStatus::Remaining { ms: 61_200 }), "1:02");
        assert_eq!(label(SleepStatus::Remaining { ms: 15 * 60_000 }), "15:00");
        assert_eq!(label(SleepStatus::EndOfTrack), "End of track");
    }
}
