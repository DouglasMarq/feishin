use mpris_server::zbus::fdo;
use mpris_server::{PlaybackStatus as MprisStatus, Time};

use crate::PlaybackStatus;

pub(super) fn not_supported(feature: &str) -> fdo::Error {
    fdo::Error::NotSupported(format!("{feature} is not supported"))
}

pub(super) fn time_from_ms(milliseconds: u64) -> Time {
    Time::from_millis(i64::try_from(milliseconds).unwrap_or(i64::MAX))
}

pub(super) fn split_status(status: PlaybackStatus) -> (MprisStatus, u64) {
    match status {
        PlaybackStatus::Playing { position_ms } => (MprisStatus::Playing, position_ms),
        PlaybackStatus::Paused { position_ms } => (MprisStatus::Paused, position_ms),
        PlaybackStatus::Stopped => (MprisStatus::Stopped, 0),
    }
}

pub(super) fn volume_from_mpris(volume: f64) -> f64 {
    if volume.is_nan() {
        return 0.0;
    }
    (volume * 100.0).round().clamp(0.0, 100.0) / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_mapping() {
        assert_eq!(
            split_status(PlaybackStatus::Playing { position_ms: 5 }),
            (MprisStatus::Playing, 5)
        );
        assert_eq!(
            split_status(PlaybackStatus::Paused { position_ms: 7 }),
            (MprisStatus::Paused, 7)
        );
        assert_eq!(
            split_status(PlaybackStatus::Stopped),
            (MprisStatus::Stopped, 0)
        );
    }

    #[test]
    fn volume_is_rounded_and_clamped() {
        assert_eq!(volume_from_mpris(0.456), 0.46);
        assert_eq!(volume_from_mpris(1.7), 1.0);
        assert_eq!(volume_from_mpris(-0.2), 0.0);
        assert_eq!(volume_from_mpris(f64::NAN), 0.0);
    }
}
