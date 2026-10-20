use super::Controller;

pub const MAX_VOLUME: u8 = 100;

impl Controller {
    pub(super) fn set_volume(&mut self, volume: u8) {
        let volume = volume.min(MAX_VOLUME);
        self.settings.playback.volume_percent = volume;
        if let Err(error) = self.deps.audio.set_volume(volume) {
            tracing::warn!("setting volume failed: {error:#}");
        }
        if volume > 0 && self.settings.playback.is_muted {
            self.set_muted(false);
            return;
        }
        self.update_media_volume();
        self.publish_volume();
    }

    pub(super) fn toggle_mute(&mut self) {
        self.set_muted(!self.settings.playback.is_muted);
    }

    fn set_muted(&mut self, is_muted: bool) {
        self.settings.playback.is_muted = is_muted;
        if let Err(error) = self.deps.audio.set_muted(is_muted) {
            tracing::warn!("muting failed: {error:#}");
        }
        self.update_media_volume();
        self.publish_volume();
    }
}
