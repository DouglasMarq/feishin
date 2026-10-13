const SUBMIT_PERCENT: u64 = 50;
const SUBMIT_AFTER_MS: u64 = 4 * 60 * 1000;
const TRACK_BEGIN_MS: u64 = 5_000;
const MAX_LISTEN_DELTA_MS: u64 = 5_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submission {
    pub song_id: String,
    pub timestamp_ms: i64,
}

#[derive(Debug, Default)]
pub struct ScrobbleTracker {
    song_id: Option<String>,
    duration_ms: u64,
    started_at_ms: i64,
    listened_ms: u64,
    last_sample_ms: Option<u64>,
    submitted: bool,
}

impl ScrobbleTracker {
    pub fn start(&mut self, song_id: &str, duration_ms: u64, now_ms: i64) {
        *self = Self {
            song_id: Some(song_id.to_owned()),
            duration_ms,
            started_at_ms: now_ms,
            ..Self::default()
        };
    }

    pub fn stop(&mut self) {
        *self = Self::default();
    }

    pub fn position(&mut self, position_ms: u64, playing: bool) -> Option<Submission> {
        let song_id = self.song_id.clone()?;
        let previous_sample_ms = self.last_sample_ms;
        self.last_sample_ms = Some(position_ms);
        if !playing {
            return None;
        }
        if let Some(previous_sample_ms) = previous_sample_ms {
            self.account_listening(previous_sample_ms, position_ms);
        }
        if self.submitted || !self.eligible() {
            return None;
        }
        self.submitted = true;
        Some(Submission {
            song_id,
            timestamp_ms: self.started_at_ms,
        })
    }

    fn account_listening(&mut self, previous_sample_ms: u64, position_ms: u64) {
        let restarted = position_ms < previous_sample_ms
            && position_ms < TRACK_BEGIN_MS
            && previous_sample_ms >= TRACK_BEGIN_MS;
        if restarted {
            self.listened_ms = 0;
            self.submitted = false;
            return;
        }
        let advanced_ms = position_ms.saturating_sub(previous_sample_ms);
        if advanced_ms > 0 && advanced_ms <= MAX_LISTEN_DELTA_MS {
            self.listened_ms += advanced_ms;
        }
    }

    fn eligible(&self) -> bool {
        let by_percent =
            self.duration_ms > 0 && self.listened_ms * 100 >= self.duration_ms * SUBMIT_PERCENT;
        by_percent || self.listened_ms >= SUBMIT_AFTER_MS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(tracker: &mut ScrobbleTracker, until_ms: u64) -> Vec<Submission> {
        (0..=until_ms / 250)
            .filter_map(|sample| tracker.position(sample * 250, true))
            .collect()
    }

    #[test]
    fn submits_at_half_of_a_short_track() {
        let mut tracker = ScrobbleTracker::default();
        tracker.start("a", 60_000, 1);
        assert!(feed(&mut tracker, 29_750).is_empty());
        assert_eq!(
            tracker.position(30_000, true),
            Some(Submission {
                song_id: "a".into(),
                timestamp_ms: 1
            })
        );
        assert!(tracker.position(30_250, true).is_none());
    }

    #[test]
    fn submits_after_four_minutes_of_a_long_track() {
        let mut tracker = ScrobbleTracker::default();
        tracker.start("a", 20 * 60_000, 0);
        assert!(feed(&mut tracker, 239_750).is_empty());
        assert!(tracker.position(240_000, true).is_some());
    }

    #[test]
    fn seeks_and_pauses_do_not_count_as_listening() {
        let mut tracker = ScrobbleTracker::default();
        tracker.start("a", 60_000, 0);
        tracker.position(0, true);
        assert!(tracker.position(50_000, true).is_none());
        assert!(tracker.position(51_000, false).is_none());
        assert!(tracker.position(55_000, false).is_none());
        assert_eq!(tracker.listened_ms, 0);
    }

    #[test]
    fn restart_resets_listen_time() {
        let mut tracker = ScrobbleTracker::default();
        tracker.start("a", 60_000, 0);
        feed(&mut tracker, 20_000);
        tracker.position(0, true);
        assert_eq!(tracker.listened_ms, 0);
    }
}
