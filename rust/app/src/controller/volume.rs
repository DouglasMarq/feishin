use super::Controller;

const MAX_VOLUME: u8 = 100;

impl Controller {
    pub(super) fn set_volume(&mut self, volume: u8) {
        let volume = volume.min(MAX_VOLUME);
        self.settings.playback.volume = volume;
        if let Err(error) = self.deps.audio.set_volume(volume) {
            tracing::warn!("setting volume failed: {error:#}");
        }
        if volume > 0 && self.settings.playback.muted {
            self.set_muted(false);
            return;
        }
        self.update_media_volume();
        self.publish_volume();
    }

    pub(super) fn toggle_mute(&mut self) {
        self.set_muted(!self.settings.playback.muted);
    }

    fn set_muted(&mut self, muted: bool) {
        self.settings.playback.muted = muted;
        if let Err(error) = self.deps.audio.set_muted(muted) {
            tracing::warn!("muting failed: {error:#}");
        }
        self.update_media_volume();
        self.publish_volume();
    }
}
