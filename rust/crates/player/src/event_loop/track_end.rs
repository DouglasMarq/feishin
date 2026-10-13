use feishin_core::queue::Uid;

use crate::PlayerEvent;
use crate::ffi::{self, EndReason};

pub(super) fn track_end_event(uid: Option<Uid>, reason: EndReason) -> Option<PlayerEvent> {
    match reason {
        EndReason::Eof => uid.map(|uid| PlayerEvent::TrackEnded { uid }),
        EndReason::Error(code) => uid.map(|uid| PlayerEvent::Error {
            uid: Some(uid),
            message: ffi::error_string(code),
        }),
        EndReason::Stop | EndReason::Quit | EndReason::Redirect | EndReason::Unknown => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_natural_end_and_errors_produce_events() {
        assert_eq!(
            track_end_event(Some(4), EndReason::Eof),
            Some(PlayerEvent::TrackEnded { uid: 4 })
        );
        assert!(matches!(
            track_end_event(Some(4), EndReason::Error(-13)),
            Some(PlayerEvent::Error { uid: Some(4), .. })
        ));
        for reason in [EndReason::Stop, EndReason::Quit, EndReason::Redirect] {
            assert_eq!(track_end_event(Some(4), reason), None);
        }
        assert_eq!(track_end_event(None, EndReason::Eof), None);
    }
}
