use std::sync::atomic::AtomicU64;
use std::sync::{Mutex, MutexGuard};

use feishin_api::TranscodeFormat;
use feishin_storage::settings::{
    MAX_CROSSFADE_SECONDS, PlaybackSettings, ReplayGainSetting, Settings,
};

use super::choices::{Choices, bitrate_choices, device_choices, format_choices};
use super::lyrics_sources::{LyricsSource, lyrics_sources};
use super::youtube::{chart_country_choices, cookie_browser_choices};
use crate::controller::SettingsSnapshot;

#[derive(Debug, Clone, Copy)]
pub(super) struct PlaybackForm {
    pub(super) replay_gain: ReplayGainSetting,
    pub(super) gapless: bool,
    pub(super) crossfade_seconds: i32,
    pub(super) device: i32,
    pub(super) bitrate: i32,
    pub(super) format: i32,
    pub(super) scrobble: bool,
}

#[derive(Default)]
pub(super) struct View {
    pub(super) settings: Settings,
    pub(super) devices: Choices<Option<String>>,
    pub(super) bitrates: Choices<Option<u32>>,
    pub(super) formats: Choices<Option<TranscodeFormat>>,
    pub(super) lyrics: Vec<LyricsSource>,
    pub(super) cookie_browsers: Choices<Option<feishin_youtube::CookieBrowser>>,
    pub(super) chart_countries: Choices<Option<String>>,
}

impl View {
    pub(super) fn new(snapshot: SettingsSnapshot) -> Self {
        let SettingsSnapshot { settings, devices } = snapshot;
        let playback = &settings.playback;
        Self {
            devices: device_choices(&devices, playback.audio_device.as_deref()),
            bitrates: bitrate_choices(playback.max_bitrate_kbps),
            formats: format_choices(playback.transcode_format.as_ref()),
            lyrics: lyrics_sources(&settings.integrations.lyrics_sources),
            cookie_browsers: cookie_browser_choices(settings.youtube.cookies_from_browser),
            chart_countries: chart_country_choices(
                settings.youtube.country.as_deref(),
                &feishin_youtube::music::SystemLocale::from_env(),
            ),
            settings,
        }
    }

    pub(super) fn playback(&self, form: PlaybackForm) -> PlaybackSettings {
        let current = &self.settings.playback;
        PlaybackSettings {
            replay_gain: form.replay_gain,
            gapless: form.gapless,
            crossfade_seconds: crossfade_seconds_from_form(form.crossfade_seconds),
            audio_device: self
                .devices
                .value(form.device)
                .unwrap_or_else(|| current.audio_device.clone()),
            max_bitrate_kbps: self
                .bitrates
                .value(form.bitrate)
                .unwrap_or(current.max_bitrate_kbps),
            transcode_format: self
                .formats
                .value(form.format)
                .unwrap_or_else(|| current.transcode_format.clone()),
            scrobble: form.scrobble,
            ..current.clone()
        }
    }
}

fn crossfade_seconds_from_form(seconds: i32) -> u8 {
    u8::try_from(seconds.clamp(0, i32::from(MAX_CROSSFADE_SECONDS))).unwrap_or(0)
}

#[derive(Default)]
pub(super) struct Shared {
    view: Mutex<View>,
    pub(super) generation: AtomicU64,
}

impl Shared {
    pub(super) fn view(&self) -> MutexGuard<'_, View> {
        self.view
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::settings::test_support::device;

    #[test]
    fn playback_form_maps_to_settings_and_keeps_volume() {
        let mut settings = Settings::default();
        settings.playback.volume_percent = 12;
        settings.playback.audio_device = Some("pipewire/hdmi".into());
        let view = View::new(SettingsSnapshot {
            settings,
            devices: vec![
                device("auto", "Autoselect"),
                device("pipewire/hdmi", "HDMI"),
            ],
        });
        let form = PlaybackForm {
            replay_gain: ReplayGainSetting::Album,
            gapless: false,
            crossfade_seconds: 7,
            device: 0,
            bitrate: 3,
            format: 2,
            scrobble: false,
        };
        let playback = view.playback(form);
        assert_eq!(playback.replay_gain, ReplayGainSetting::Album);
        assert!(!playback.gapless && !playback.scrobble);
        assert_eq!(playback.crossfade_seconds, 7);
        assert_eq!(playback.audio_device, None);
        assert_eq!(playback.max_bitrate_kbps, Some(192));
        assert_eq!(playback.transcode_format, TranscodeFormat::parse("opus"));
        assert_eq!(playback.volume_percent, 12);

        let out_of_range = view.playback(PlaybackForm { device: 99, ..form });
        assert_eq!(out_of_range.audio_device.as_deref(), Some("pipewire/hdmi"));

        let too_long = view.playback(PlaybackForm {
            crossfade_seconds: 99,
            ..form
        });
        assert_eq!(too_long.crossfade_seconds, MAX_CROSSFADE_SECONDS);
        let negative = view.playback(PlaybackForm {
            crossfade_seconds: -3,
            ..form
        });
        assert_eq!(negative.crossfade_seconds, 0);
    }
}
