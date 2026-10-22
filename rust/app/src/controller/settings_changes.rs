use feishin_storage::appearance::AppearanceSettings;
use feishin_storage::settings::{IntegrationSettings, PlaybackSettings, TraySettings};
use tokio::sync::oneshot;

use super::{Controller, SettingsSnapshot, UiUpdate};

fn preload_depends_on_changes(current: &PlaybackSettings, next: &PlaybackSettings) -> bool {
    next.is_gapless != current.is_gapless
        || next.crossfade().is_zero() != current.crossfade().is_zero()
        || next.max_bitrate_kbps != current.max_bitrate_kbps
        || next.transcode_format != current.transcode_format
}

impl Controller {
    pub(super) fn reply_with_settings(&self, reply: oneshot::Sender<SettingsSnapshot>) {
        let devices = self.deps.audio.audio_devices().unwrap_or_else(|error| {
            tracing::warn!("listing audio devices failed: {error:#}");
            Vec::new()
        });
        let snapshot = SettingsSnapshot {
            settings: self.settings.clone(),
            devices,
        };
        if reply.send(snapshot).is_err() {
            tracing::debug!("the settings request was dropped before the reply");
        }
    }

    pub(super) fn apply_playback(&mut self, playback: PlaybackSettings) {
        let next = PlaybackSettings {
            volume_percent: self.settings.playback.volume_percent,
            is_muted: self.settings.playback.is_muted,
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
            self.deps.ui.send(UiUpdate::Error(tr!(
                "ReplayGain could not be changed: {}",
                format!("{error:#}")
            )));
        }
        if next.audio_device != current.audio_device
            && let Err(error) = audio.set_audio_device(next.audio_device.as_deref())
        {
            tracing::warn!("switching audio device failed: {error:#}");
            self.deps.ui.send(UiUpdate::Error(tr!(
                "The audio device could not be changed: {}",
                format!("{error:#}")
            )));
        }
        if next.crossfade() != current.crossfade()
            && let Err(error) = audio.set_crossfade(next.crossfade())
        {
            tracing::warn!("changing the crossfade failed: {error:#}");
            self.deps.ui.send(UiUpdate::Error(tr!(
                "Crossfade could not be changed: {}",
                format!("{error:#}")
            )));
        }
    }

    pub(super) fn save_integrations(&mut self, integrations: IntegrationSettings) {
        self.settings.integrations = integrations;
        self.save_settings();
    }

    pub(super) fn save_interface(
        &mut self,
        is_queue_panel_open: bool,
        is_following_current_song: bool,
    ) {
        self.settings.ui.is_queue_panel_open = is_queue_panel_open;
        self.settings.ui.is_following_current_song = is_following_current_song;
        self.save_settings();
    }

    pub(super) fn save_language(&mut self, language: Option<String>) {
        self.settings.ui.language = language;
        self.save_settings();
    }

    pub(super) fn save_tray(&mut self, tray: TraySettings) {
        self.settings.ui.tray = tray;
        self.save_settings();
    }

    pub(super) fn save_appearance(&mut self, appearance: AppearanceSettings) {
        self.settings.appearance = appearance;
        self.save_settings();
    }
}
