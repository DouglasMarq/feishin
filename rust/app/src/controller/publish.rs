use feishin_core::queue::QueueItem;

use super::{Controller, QueueEntry, UiUpdate};

fn queue_entry(item: &QueueItem) -> QueueEntry {
    QueueEntry {
        uid: item.uid,
        song_id: item.track.song_id.to_string(),
        title: item.track.title.to_string(),
        artists: item.track.artist_names(),
        duration_ms: item.track.duration_ms,
    }
}

impl Controller {
    pub(super) fn publish_all(&self) {
        self.publish_volume();
        self.publish_modes();
        self.publish_queue();
        self.publish_now_playing();
        self.publish_playback();
    }

    pub(super) fn publish_now_playing(&self) {
        let track = self
            .queue
            .current()
            .map(|item| Box::new(item.track.clone()));
        self.deps.ui.send(UiUpdate::NowPlaying(track));
    }

    pub(super) fn publish_playback(&self) {
        self.deps.ui.send(UiUpdate::Playback {
            playing: self.playing,
            position_ms: self.position_ms,
            duration_ms: self.duration_ms,
        });
    }

    pub(super) fn publish_volume(&self) {
        self.deps.ui.send(UiUpdate::Volume {
            volume: self.settings.playback.volume,
            muted: self.settings.playback.muted,
        });
    }

    pub(super) fn publish_modes(&self) {
        self.deps.ui.send(UiUpdate::Modes {
            shuffle: self.queue.shuffle(),
            repeat: self.queue.repeat(),
        });
    }

    pub(super) fn publish_queue(&self) {
        let entries = self
            .queue
            .display_order()
            .into_iter()
            .map(queue_entry)
            .collect();
        self.deps.ui.send(UiUpdate::Queue {
            entries,
            current: self.queue.current().map(|item| item.uid),
        });
    }

    pub(super) fn publish_queue_current(&self) {
        self.deps.ui.send(UiUpdate::QueueCurrent(
            self.queue.current().map(|item| item.uid),
        ));
    }
}
