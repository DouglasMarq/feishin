use std::collections::VecDeque;
use std::time::Duration;

use feishin_core::queue::Uid;

use crate::crossfade::DeckSlot;

const REMEMBERED_TRACKS: usize = 4;
const REPORTED_DURATION_TOLERANCE_SECONDS: f64 = 1.5;

#[derive(Default)]
pub(crate) struct TrackDurations {
    known_seconds: VecDeque<(Uid, f64)>,
    first_playing: Option<Uid>,
    second_playing: Option<Uid>,
}

impl TrackDurations {
    pub(crate) fn remember(&mut self, uid: Uid, duration_ms: Option<u64>) {
        let Some(duration_ms) = duration_ms.filter(|milliseconds| *milliseconds > 0) else {
            return;
        };
        self.known_seconds
            .retain(|(known_uid, _)| *known_uid != uid);
        if self.known_seconds.len() >= REMEMBERED_TRACKS {
            self.known_seconds.pop_front();
        }
        self.known_seconds
            .push_back((uid, Duration::from_millis(duration_ms).as_secs_f64()));
    }

    pub(crate) fn playing(&mut self, slot: DeckSlot, uid: Uid) {
        match slot {
            DeckSlot::First => self.first_playing = Some(uid),
            DeckSlot::Second => self.second_playing = Some(uid),
        }
    }

    pub(crate) fn expected_seconds(&self, slot: DeckSlot, reported: Option<f64>) -> Option<f64> {
        let reported = reported.filter(|seconds| seconds.is_finite() && *seconds > 0.0);
        match (self.hinted_seconds(slot), reported) {
            (Some(hinted), Some(reported))
                if (reported - hinted).abs() <= REPORTED_DURATION_TOLERANCE_SECONDS =>
            {
                Some(reported)
            }
            (Some(hinted), _) => Some(hinted),
            (None, reported) => reported,
        }
    }

    fn hinted_seconds(&self, slot: DeckSlot) -> Option<f64> {
        let playing = match slot {
            DeckSlot::First => self.first_playing,
            DeckSlot::Second => self.second_playing,
        }?;
        self.known_seconds
            .iter()
            .find(|(uid, _)| *uid == playing)
            .map(|(_, seconds)| *seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expected_duration_follows_what_each_deck_plays() {
        let mut durations = TrackDurations::default();
        durations.remember(1, Some(200_000));
        durations.remember(2, Some(1_500));
        durations.remember(3, None);
        durations.remember(4, Some(0));
        assert_eq!(durations.expected_seconds(DeckSlot::First, None), None);
        durations.playing(DeckSlot::First, 1);
        durations.playing(DeckSlot::Second, 2);
        assert_eq!(
            durations.expected_seconds(DeckSlot::First, None),
            Some(200.0)
        );
        assert_eq!(
            durations.expected_seconds(DeckSlot::Second, None),
            Some(1.5)
        );
        durations.playing(DeckSlot::First, 3);
        assert_eq!(durations.expected_seconds(DeckSlot::First, None), None);
    }

    #[test]
    fn precise_reported_duration_wins_only_when_close_to_the_hint() {
        let mut durations = TrackDurations::default();
        durations.remember(1, Some(200_000));
        durations.playing(DeckSlot::First, 1);
        let expected = |reported| durations.expected_seconds(DeckSlot::First, reported);
        assert_eq!(expected(Some(200.7)), Some(200.7));
        assert_eq!(expected(Some(199.2)), Some(199.2));
        assert_eq!(expected(Some(30.0)), Some(200.0));
        assert_eq!(expected(Some(f64::NAN)), Some(200.0));
        assert_eq!(expected(None), Some(200.0));
        assert_eq!(
            durations.expected_seconds(DeckSlot::Second, Some(42.0)),
            Some(42.0)
        );
        assert_eq!(
            durations.expected_seconds(DeckSlot::Second, Some(-1.0)),
            None
        );
    }

    #[test]
    fn only_a_few_recent_durations_are_kept() {
        let mut durations = TrackDurations::default();
        for uid in 1..=10 {
            durations.remember(uid, Some(uid * 1_000));
        }
        durations.remember(9, Some(9_500));
        durations.playing(DeckSlot::First, 9);
        assert_eq!(durations.expected_seconds(DeckSlot::First, None), Some(9.5));
        durations.playing(DeckSlot::First, 6);
        assert_eq!(durations.expected_seconds(DeckSlot::First, None), None);
        assert_eq!(durations.known_seconds.len(), REMEMBERED_TRACKS);
    }
}
