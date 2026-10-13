use feishin_core::queue::QueueTrack;

use super::{Controller, UiUpdate};

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
            .map(|item| Box::new(QueueTrack::clone(&item.track)));
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
        self.deps.ui.send(UiUpdate::Queue(self.queue.view()));
    }

    pub(super) fn publish_queue_current(&self) {
        self.publish_queue();
    }
}
