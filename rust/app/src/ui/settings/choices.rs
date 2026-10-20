use feishin_api::TranscodeFormat;
use feishin_player::AudioDevice;
use feishin_storage::settings::ReplayGainSetting;

use crate::ui::ReplayGain;

const BITRATES_KBPS: [u32; 7] = [320, 256, 192, 160, 128, 96, 64];
const FORMATS: [&str; 5] = ["mp3", "opus", "aac", "ogg", "flac"];
const MPV_SYSTEM_DEFAULT_DEVICE: &str = "auto";

#[derive(Debug, Default, PartialEq)]
pub(super) struct Choices<T> {
    pub(super) labels: Vec<String>,
    values: Vec<T>,
    pub(super) selected: usize,
}

impl<T: PartialEq> Choices<T> {
    pub(super) fn new(options: Vec<(String, T)>, current: &T) -> Self {
        let (labels, values): (Vec<_>, Vec<_>) = options.into_iter().unzip();
        let selected = values
            .iter()
            .position(|value| value == current)
            .unwrap_or(0);
        Self {
            labels,
            values,
            selected,
        }
    }
}

impl<T: Clone> Choices<T> {
    pub(super) fn value(&self, index: i32) -> Option<T> {
        usize::try_from(index)
            .ok()
            .and_then(|position| self.values.get(position))
            .cloned()
    }
}

pub(super) fn bitrate_choices(current: Option<u32>) -> Choices<Option<u32>> {
    let mut values: Vec<Option<u32>> = std::iter::once(None)
        .chain(BITRATES_KBPS.map(Some))
        .collect();
    if !values.contains(&current) {
        values.push(current);
    }
    let options = values
        .into_iter()
        .map(|value| (bitrate_label(value), value))
        .collect();
    Choices::new(options, &current)
}

fn bitrate_label(bitrate_kbps: Option<u32>) -> String {
    bitrate_kbps.map_or_else(|| tr!("Original"), |kbps| tr!("{} kbps", kbps))
}

pub(super) fn format_choices(
    current: Option<&TranscodeFormat>,
) -> Choices<Option<TranscodeFormat>> {
    let current = current.cloned();
    let mut values: Vec<Option<TranscodeFormat>> = std::iter::once(None)
        .chain(FORMATS.map(TranscodeFormat::parse))
        .collect();
    if !values.contains(&current) {
        values.push(current.clone());
    }
    let options = values
        .into_iter()
        .map(|value| (format_label(value.as_ref()), value))
        .collect();
    Choices::new(options, &current)
}

fn format_label(format: Option<&TranscodeFormat>) -> String {
    format.map_or_else(|| tr!("Original"), |format| format.as_str().to_owned())
}

pub(super) fn device_choices(
    devices: &[AudioDevice],
    current: Option<&str>,
) -> Choices<Option<String>> {
    let mut options = Vec::new();
    if !devices
        .iter()
        .any(|device| device.name == MPV_SYSTEM_DEFAULT_DEVICE)
    {
        options.push((tr!("System default"), None));
    }
    options.extend(devices.iter().map(device_option));
    let current = current
        .filter(|name| *name != MPV_SYSTEM_DEFAULT_DEVICE)
        .map(str::to_owned);
    if let Some(name) = &current
        && !options
            .iter()
            .any(|(_, value)| value.as_ref() == Some(name))
    {
        options.push((tr!("{} (unavailable)", name), current.clone()));
    }
    Choices::new(options, &current)
}

fn device_option(device: &AudioDevice) -> (String, Option<String>) {
    let label = Some(&device.description)
        .filter(|description| !description.is_empty())
        .unwrap_or(&device.name)
        .clone();
    let value = (device.name != MPV_SYSTEM_DEFAULT_DEVICE).then(|| device.name.clone());
    (label, value)
}

pub(super) fn replay_gain_choice(setting: ReplayGainSetting) -> ReplayGain {
    match setting {
        ReplayGainSetting::Off => ReplayGain::Off,
        ReplayGainSetting::Track => ReplayGain::Track,
        ReplayGainSetting::Album => ReplayGain::Album,
    }
}

pub(super) fn replay_gain_setting(choice: ReplayGain) -> ReplayGainSetting {
    match choice {
        ReplayGain::Off => ReplayGainSetting::Off,
        ReplayGain::Track => ReplayGainSetting::Track,
        ReplayGain::Album => ReplayGainSetting::Album,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::settings::display::slint_index;
    use crate::ui::settings::test_support::device;

    #[test]
    fn bitrates_offer_original_and_keep_custom_values() {
        let choices = bitrate_choices(None);
        assert_eq!(choices.labels[0], "Original");
        assert_eq!(choices.selected, 0);
        assert_eq!(choices.value(1), Some(Some(320)));
        let custom = bitrate_choices(Some(500));
        assert_eq!(custom.labels[custom.selected], "500 kbps");
        assert_eq!(bitrate_choices(Some(128)).labels[5], "128 kbps");
        assert_eq!(custom.value(-1), None);
    }

    #[test]
    fn formats_are_normalized_and_custom_ones_kept() {
        let choices = format_choices(TranscodeFormat::parse(" OPUS ").as_ref());
        assert_eq!(choices.labels[choices.selected], "opus");
        let alac = TranscodeFormat::parse("alac");
        let custom = format_choices(alac.as_ref());
        assert_eq!(custom.value(slint_index(custom.selected)), Some(alac));
        assert_eq!(format_choices(None).selected, 0);
    }

    #[test]
    fn devices_map_auto_to_the_default_and_keep_missing_ones() {
        let devices = [
            device("auto", "Autoselect device"),
            device("pipewire/hdmi", "HDMI"),
            device("alsa/raw", ""),
        ];
        let choices = device_choices(&devices, None);
        assert_eq!(choices.labels, ["Autoselect device", "HDMI", "alsa/raw"]);
        assert_eq!(choices.selected, 0);
        assert_eq!(choices.value(0), Some(None));

        let selected = device_choices(&devices, Some("pipewire/hdmi"));
        assert_eq!(selected.selected, 1);

        let missing = device_choices(&devices[1..], Some("pulse/gone"));
        assert_eq!(missing.labels[0], "System default");
        assert_eq!(missing.labels[missing.selected], "pulse/gone (unavailable)");
    }

    #[test]
    fn replay_gain_round_trips() {
        for setting in [
            ReplayGainSetting::Off,
            ReplayGainSetting::Track,
            ReplayGainSetting::Album,
        ] {
            assert_eq!(replay_gain_setting(replay_gain_choice(setting)), setting);
        }
    }
}
