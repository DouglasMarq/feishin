use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const DEFAULT_DARK_THEME: &str = "defaultDark";
pub const DEFAULT_LIGHT_THEME: &str = "defaultLight";
const DEFAULT_ACCENT: AccentColor = AccentColor::new(53, 116, 252);
const HEX_RADIX: u32 = 16;
const HEX_COLOR_DIGITS: usize = 6;
const HEX_DIGITS_PER_CHANNEL: usize = 2;
const RED_DIGITS_OFFSET: usize = 0;
const GREEN_DIGITS_OFFSET: usize = 2;
const BLUE_DIGITS_OFFSET: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ThemeId(String);

impl ThemeId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn default_dark() -> Self {
        Self::new(DEFAULT_DARK_THEME)
    }

    pub fn default_light() -> Self {
        Self::new(DEFAULT_LIGHT_THEME)
    }
}

impl Default for ThemeId {
    fn default() -> Self {
        Self::default_dark()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AccentColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl AccentColor {
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }
}

impl Default for AccentColor {
    fn default() -> Self {
        DEFAULT_ACCENT
    }
}

impl fmt::Display for AccentColor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "#{:02x}{:02x}{:02x}",
            self.red, self.green, self.blue
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("an accent color looks like #3574fc")]
pub struct InvalidAccentColor;

fn hex_channel(digits: &str, position: usize) -> Result<u8, InvalidAccentColor> {
    let channel = digits
        .get(position..position + HEX_DIGITS_PER_CHANNEL)
        .ok_or(InvalidAccentColor)?;
    u8::from_str_radix(channel, HEX_RADIX).map_err(|error| {
        tracing::debug!("the accent color channel {channel:?} is not hex: {error}");
        InvalidAccentColor
    })
}

impl FromStr for AccentColor {
    type Err = InvalidAccentColor;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let trimmed = text.trim();
        let digits = trimmed.strip_prefix('#').unwrap_or(trimmed);
        if digits.len() != HEX_COLOR_DIGITS
            || !digits.chars().all(|digit| digit.is_ascii_hexdigit())
        {
            return Err(InvalidAccentColor);
        }
        Ok(Self::new(
            hex_channel(digits, RED_DIGITS_OFFSET)?,
            hex_channel(digits, GREEN_DIGITS_OFFSET)?,
            hex_channel(digits, BLUE_DIGITS_OFFSET)?,
        ))
    }
}

impl Serialize for AccentColor {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for AccentColor {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Ok(text.parse().unwrap_or_else(|error| {
            tracing::warn!("ignoring the saved accent color {text:?}: {error}");
            Self::default()
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceSettings {
    pub theme: ThemeId,
    #[serde(rename = "follow_system")]
    pub is_following_system: bool,
    pub dark_theme: ThemeId,
    pub light_theme: ThemeId,
    #[serde(rename = "use_theme_accent")]
    pub is_using_theme_accent: bool,
    pub accent: AccentColor,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: ThemeId::default_dark(),
            is_following_system: false,
            dark_theme: ThemeId::default_dark(),
            light_theme: ThemeId::default_light(),
            is_using_theme_accent: true,
            accent: AccentColor::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    struct Wrapper {
        appearance: AppearanceSettings,
    }

    #[test]
    fn accent_colors_parse_with_or_without_the_hash() {
        assert_eq!("#3574fc".parse(), Ok(AccentColor::new(53, 116, 252)));
        assert_eq!("F0AA16".parse(), Ok(AccentColor::new(240, 170, 22)));
        assert_eq!(" #1db954 ".parse(), Ok(AccentColor::new(29, 185, 84)));
    }

    #[test]
    fn malformed_accent_colors_are_rejected() {
        for text in [
            "", "#123", "#12345g", "#1234567", "#ééé", "blue", "#+1+2+3", "+a+b+c",
        ] {
            assert_eq!(
                text.parse::<AccentColor>(),
                Err(InvalidAccentColor),
                "{text}"
            );
        }
    }

    #[test]
    fn accent_colors_display_as_lowercase_hex() {
        assert_eq!(AccentColor::new(214, 81, 63).to_string(), "#d6513f");
    }

    #[test]
    fn appearance_round_trips_through_toml() {
        let wrapper = Wrapper {
            appearance: AppearanceSettings {
                theme: ThemeId::new("nord"),
                is_following_system: true,
                dark_theme: ThemeId::new("dracula"),
                light_theme: ThemeId::new("githubLight"),
                is_using_theme_accent: false,
                accent: AccentColor::new(170, 110, 216),
            },
        };
        let text = toml::to_string_pretty(&wrapper).unwrap();
        assert!(text.contains("accent = \"#aa6ed8\""), "{text}");
        assert_eq!(toml::from_str::<Wrapper>(&text).unwrap(), wrapper);
    }

    #[test]
    fn missing_appearance_keeps_the_default_dark_look() {
        let wrapper: Wrapper = toml::from_str("").unwrap();
        assert_eq!(wrapper.appearance, AppearanceSettings::default());
        assert_eq!(wrapper.appearance.theme.as_str(), DEFAULT_DARK_THEME);
        assert!(wrapper.appearance.is_using_theme_accent);
    }

    #[test]
    fn a_bad_accent_falls_back_instead_of_failing_the_file() {
        let wrapper: Wrapper =
            toml::from_str("[appearance]\ntheme = \"nord\"\naccent = \"oops\"\n").unwrap();
        assert_eq!(wrapper.appearance.theme, ThemeId::new("nord"));
        assert_eq!(wrapper.appearance.accent, AccentColor::default());
    }
}
