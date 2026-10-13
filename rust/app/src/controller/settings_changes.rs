use feishin_storage::settings::{IntegrationSettings, PlaybackSettings};
use tokio::sync::oneshot;

use super::{Controller, SettingsSnapshot};

fn preload_depends_on_changes(current: &PlaybackSettings, next: &PlaybackSettings) -> bool {
    next.gapless != current.gapless
        || next.max_bitrate_kbps != current.max_bitrate_kbps
        || next.transcode_format != current.transcode_format
}

impl Controller {
    pub(super) fn reply_with_settings(&self, reply: oneshot::Sender<SettingsSnapshot>) {
        let devices = self.deps.audio.audio_devices().unwrap_or_else(|error| {
            tracing::warn!("listing audio devices failed: {error:#}");
            Vec::new()
        });
        let _ = reply.send(SettingsSnapshot {
            settings: self.settings.clone(),
            devices,
        });
    }

    pub(super) fn apply_playback(&mut self, playback: PlaybackSettings) {
        let next = PlaybackSettings {
            volume: self.settings.playback.volume,
            muted: self.settings.playback.muted,
            ..playback
        };
        self.apply_output_changes(&next);
        let preload_is_stale = preload_depends_on_changes(&self.settings.playback, &next);
        self.settings.playback = next;
        if preload_is_stale {
            self.discard_preload();
        }
        self.refresh_preload();
        self.save_settings();
    }

    fn apply_output_changes(&self, next: &PlaybackSettings) {
        let current = &self.settings.playback;
        let audio = &self.deps.audio;
        if next.replay_gain != current.replay_gain
            && let Err(error) = audio.set_replay_gain(next.replay_gain)
        {
            tracing::warn!("setting ReplayGain failed: {error:#}");
        }
        if next.audio_device != current.audio_device
            && let Err(error) = audio.set_audio_device(next.audio_device.as_deref())
        {
            tracing::warn!("switching audio device failed: {error:#}");
        }
    }

    pub(super) fn save_integrations(&mut self, integrations: IntegrationSettings) {
        self.settings.integrations = integrations;
        self.save_settings();
    }

    pub(super) fn save_interface(&mut self, queue_panel_open: bool, follow_current_song: bool) {
        self.settings.ui.queue_panel_open = queue_panel_open;
        self.settings.ui.follow_current_song = follow_current_song;
        self.save_settings();
    }
}
