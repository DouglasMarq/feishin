use super::{AUTOMATIC_AUDIO_DEVICE, MAX_VOLUME};
use crate::ReplayGainMode;
use crate::crossfade::Gain;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DeckSetting {
    Paused(bool),
    Volume(u8),
    Muted(bool),
    ReplayGain(ReplayGainMode),
    AudioDevice(Option<String>),
    Gain(Gain),
}

impl DeckSetting {
    pub(super) fn property_name(&self) -> &'static str {
        match self {
            DeckSetting::Paused(_) => "pause",
            DeckSetting::Volume(_) => "volume",
            DeckSetting::Muted(_) => "mute",
            DeckSetting::ReplayGain(_) => "replaygain",
            DeckSetting::AudioDevice(_) => "audio-device",
            DeckSetting::Gain(_) => "volume-gain",
        }
    }

    pub(super) fn mpv_value(&self) -> String {
        match self {
            DeckSetting::Paused(is_enabled) | DeckSetting::Muted(is_enabled) => {
                mpv_flag(*is_enabled).into()
            }
            DeckSetting::Volume(volume) => volume.min(&MAX_VOLUME).to_string(),
            DeckSetting::ReplayGain(mode) => mode.mpv_value().into(),
            DeckSetting::AudioDevice(device) => {
                device.as_deref().unwrap_or(AUTOMATIC_AUDIO_DEVICE).into()
            }
            DeckSetting::Gain(gain) => format!("{:.3}", gain.decibels()),
        }
    }
}

pub(super) fn mpv_flag(is_enabled: bool) -> &'static str {
    match is_enabled {
        true => "yes",
        false => "no",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_map_to_mpv_properties() {
        let cases = [
            (DeckSetting::Paused(true), "pause", "yes"),
            (DeckSetting::Muted(false), "mute", "no"),
            (DeckSetting::Volume(250), "volume", "100"),
            (
                DeckSetting::ReplayGain(ReplayGainMode::Album),
                "replaygain",
                "album",
            ),
            (DeckSetting::AudioDevice(None), "audio-device", "auto"),
            (
                DeckSetting::AudioDevice(Some("pipewire/hdmi".into())),
                "audio-device",
                "pipewire/hdmi",
            ),
            (DeckSetting::Gain(Gain::SILENT), "volume-gain", "-96.000"),
            (DeckSetting::Gain(Gain::UNITY), "volume-gain", "0.000"),
        ];
        for (setting, name, value) in cases {
            assert_eq!(setting.property_name(), name);
            assert_eq!(setting.mpv_value(), value);
        }
    }
}
