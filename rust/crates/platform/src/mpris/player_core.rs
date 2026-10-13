use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use mpris_server::{PlaybackStatus as MprisStatus, Property, Time};
use tokio::sync::mpsc;

use super::conversions::{split_status, time_from_ms};
use super::metadata::metadata_for;
use super::snapshot::PlaybackSnapshot;
use super::{EventCallback, Update};
use crate::{MediaControlEvent, NowPlaying, PlaybackStatus};

const SEEK_DETECT_THRESHOLD_MS: u64 = 1000;

#[derive(Default)]
pub(super) struct AppliedUpdate {
    pub(super) properties: Vec<Property>,
    pub(super) seeked_to: Option<Time>,
}

impl AppliedUpdate {
    fn properties(properties: Vec<Property>) -> Self {
        Self {
            properties,
            seeked_to: None,
        }
    }
}

pub(super) struct MprisPlayerCore {
    pub(super) identity: String,
    pub(super) bus_suffix: String,
    pub(super) snapshot: Mutex<PlaybackSnapshot>,
    pub(super) on_event: EventCallback,
    pub(super) updates: mpsc::WeakUnboundedSender<Update>,
}

impl MprisPlayerCore {
    pub(super) fn snapshot(&self) -> MutexGuard<'_, PlaybackSnapshot> {
        self.snapshot.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(super) fn apply(&self, update: Update, now: Instant) -> AppliedUpdate {
        match update {
            Update::Metadata(now_playing) => self.apply_metadata(&now_playing),
            Update::Status(status) => self.apply_status(status, now),
            Update::Volume(volume) => self.apply_volume(volume),
            Update::Emit(properties) => AppliedUpdate::properties(properties),
        }
    }

    pub(super) fn emit(&self, event: MediaControlEvent) {
        (self.on_event)(event);
    }

    pub(super) fn notify(&self, properties: Vec<Property>) {
        if let Some(updates) = self.updates.upgrade() {
            let _ = updates.send(Update::Emit(properties));
        }
    }

    pub(super) fn update_status(&self, next: impl FnOnce(MprisStatus) -> MprisStatus) {
        let now = Instant::now();
        let mut snapshot = self.snapshot();
        let status = next(snapshot.status);
        if status == snapshot.status {
            return;
        }
        let position_ms = snapshot.position_ms(now);
        snapshot.set_status(status, position_ms, now);
        drop(snapshot);
        self.notify(vec![Property::PlaybackStatus(status)]);
    }

    pub(super) fn transport(
        &self,
        event: MediaControlEvent,
        next: impl FnOnce(MprisStatus) -> MprisStatus,
    ) {
        if !self.has_track() {
            return;
        }
        self.emit(event);
        self.update_status(next);
    }

    fn has_track(&self) -> bool {
        self.snapshot().has_track()
    }

    fn apply_metadata(&self, now_playing: &NowPlaying) -> AppliedUpdate {
        let metadata = metadata_for(now_playing);
        self.snapshot().metadata = metadata.clone();
        AppliedUpdate::properties(vec![Property::Metadata(metadata)])
    }

    fn apply_status(&self, status: PlaybackStatus, now: Instant) -> AppliedUpdate {
        let mut snapshot = self.snapshot();
        let (status, position_ms) = split_status(status);
        let expected_position_ms = snapshot.position_ms(now);
        let seeked = status != MprisStatus::Stopped
            && expected_position_ms.abs_diff(position_ms) > SEEK_DETECT_THRESHOLD_MS;
        let status_changed = snapshot.status != status;
        snapshot.set_status(status, position_ms, now);
        AppliedUpdate {
            properties: status_changed
                .then_some(Property::PlaybackStatus(status))
                .into_iter()
                .collect(),
            seeked_to: seeked.then(|| time_from_ms(position_ms)),
        }
    }

    fn apply_volume(&self, volume: f64) -> AppliedUpdate {
        let volume = volume.clamp(0.0, 1.0);
        let mut snapshot = self.snapshot();
        if (snapshot.volume - volume).abs() < f64::EPSILON {
            return AppliedUpdate::default();
        }
        snapshot.volume = volume;
        AppliedUpdate::properties(vec![Property::Volume(volume)])
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use super::*;

    fn song() -> NowPlaying {
        NowPlaying {
            track_id: "a1b2-c3d4.e5".to_owned(),
            title: "Song".to_owned(),
            artists: vec!["Artist A".to_owned(), "Artist B".to_owned()],
            album: Some("Album".to_owned()),
            duration_ms: Some(215_500),
            cover_url: Some("https://example.com/cover.jpg".to_owned()),
        }
    }

    type UpdateChannel = (
        mpsc::UnboundedSender<Update>,
        mpsc::UnboundedReceiver<Update>,
    );

    fn player_core(on_event: EventCallback) -> (MprisPlayerCore, UpdateChannel) {
        let (update_sender, update_receiver) = mpsc::unbounded_channel();
        let core = MprisPlayerCore {
            identity: "Feishin".to_owned(),
            bus_suffix: "feishin".to_owned(),
            snapshot: Mutex::new(PlaybackSnapshot::new()),
            on_event,
            updates: update_sender.downgrade(),
        };
        (core, (update_sender, update_receiver))
    }

    #[test]
    fn status_update_emits_changes_and_seeks() {
        let (core, _channel) = player_core(Box::new(|_| {}));
        let now = Instant::now();

        let applied = core.apply(
            Update::Status(PlaybackStatus::Playing { position_ms: 0 }),
            now,
        );
        assert!(matches!(
            applied.properties.as_slice(),
            [Property::PlaybackStatus(MprisStatus::Playing)]
        ));
        assert_eq!(applied.seeked_to, None);

        let applied = core.apply(
            Update::Status(PlaybackStatus::Playing { position_ms: 1_200 }),
            now + Duration::from_millis(1_000),
        );
        assert!(applied.properties.is_empty());
        assert_eq!(applied.seeked_to, None);

        let applied = core.apply(
            Update::Status(PlaybackStatus::Playing {
                position_ms: 60_000,
            }),
            now + Duration::from_millis(2_000),
        );
        assert_eq!(applied.seeked_to, Some(Time::from_millis(60_000)));
    }

    #[test]
    fn volume_update_only_emits_on_change() {
        let (core, _channel) = player_core(Box::new(|_| {}));
        let now = Instant::now();
        let applied = core.apply(Update::Volume(1.0), now);
        assert!(applied.properties.is_empty());
        let applied = core.apply(Update::Volume(0.5), now);
        assert!(
            matches!(applied.properties.as_slice(), [Property::Volume(volume)] if *volume == 0.5)
        );
    }

    #[test]
    fn transport_requires_a_track() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let event_sink = Arc::clone(&events);
        let (core, (_update_sender, mut update_receiver)) = player_core(Box::new(move |event| {
            event_sink.lock().unwrap().push(event);
        }));

        core.transport(MediaControlEvent::Toggle, |_| MprisStatus::Playing);
        assert!(events.lock().unwrap().is_empty());
        assert_eq!(core.snapshot().status, MprisStatus::Stopped);

        core.apply(Update::Metadata(song()), Instant::now());
        core.transport(MediaControlEvent::Toggle, |_| MprisStatus::Playing);
        assert_eq!(*events.lock().unwrap(), vec![MediaControlEvent::Toggle]);
        assert_eq!(core.snapshot().status, MprisStatus::Playing);
        assert!(matches!(
            update_receiver.try_recv(),
            Ok(Update::Emit(properties)) if matches!(properties.as_slice(), [Property::PlaybackStatus(MprisStatus::Playing)])
        ));
    }
}
