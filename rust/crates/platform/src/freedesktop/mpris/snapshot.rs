use std::time::Instant;

use mpris_server::{Metadata, PlaybackStatus as MprisStatus};

pub(super) struct PlaybackSnapshot {
    pub(super) metadata: Metadata,
    pub(super) status: MprisStatus,
    pub(super) volume: f64,
    pub(super) is_live_stream: bool,
    position_ms: u64,
    position_at: Instant,
}

impl PlaybackSnapshot {
    pub(super) fn new() -> Self {
        Self {
            metadata: Metadata::new(),
            status: MprisStatus::Stopped,
            position_ms: 0,
            position_at: Instant::now(),
            volume: 1.0,
            is_live_stream: false,
        }
    }

    pub(super) fn has_track(&self) -> bool {
        self.metadata.trackid().is_some()
    }

    pub(super) fn position_ms(&self, now: Instant) -> u64 {
        match self.status {
            MprisStatus::Playing => {
                let elapsed = now.saturating_duration_since(self.position_at).as_millis();
                self.position_ms
                    .saturating_add(u64::try_from(elapsed).unwrap_or(u64::MAX))
            }
            MprisStatus::Paused | MprisStatus::Stopped => self.position_ms,
        }
    }

    pub(super) fn set_status(&mut self, status: MprisStatus, position_ms: u64, now: Instant) {
        self.status = status;
        self.position_ms = position_ms;
        self.position_at = now;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn position_advances_only_while_playing() {
        let start = Instant::now();
        let mut snapshot = PlaybackSnapshot::new();
        snapshot.set_status(MprisStatus::Playing, 1_000, start);
        assert_eq!(
            snapshot.position_ms(start + Duration::from_millis(500)),
            1_500
        );
        snapshot.set_status(MprisStatus::Paused, 2_000, start);
        assert_eq!(snapshot.position_ms(start + Duration::from_secs(10)), 2_000);
    }
}
