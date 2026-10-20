mod sink;
mod ticker;
mod wiring;

use tokio::sync::mpsc::UnboundedSender;
use tokio::task::AbortHandle;

use crate::controller::Command;
pub use sink::SleepTimerUi;
use ticker::spawn_ticker;
pub use wiring::install;

const MS_PER_MINUTE: u64 = 60_000;

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
}
