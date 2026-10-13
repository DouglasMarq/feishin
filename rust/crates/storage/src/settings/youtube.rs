use feishin_youtube::music::chart_country;
use feishin_youtube::{CookieBrowser, DEFAULT_PROGRAM};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct YouTubeSettings {
    pub yt_dlp_path: Option<String>,
    #[serde(deserialize_with = "known_browser")]
    pub cookies_from_browser: Option<CookieBrowser>,
    #[serde(deserialize_with = "known_country")]
    pub country: Option<String>,
}

impl YouTubeSettings {
    pub fn yt_dlp_program(&self) -> &str {
        self.yt_dlp_path
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .unwrap_or(DEFAULT_PROGRAM)
    }
}

fn known_browser<'de, BrowserDeserializer: Deserializer<'de>>(
    deserializer: BrowserDeserializer,
) -> Result<Option<CookieBrowser>, BrowserDeserializer::Error> {
    let name = Option::<String>::deserialize(deserializer)?;
    Ok(name.and_then(|name| {
        let browser = CookieBrowser::from_yt_dlp_name(&name);
        if browser.is_none() {
            tracing::warn!("ignoring the unknown cookie browser {name}");
        }
        browser
    }))
}

fn known_country<'de, CountryDeserializer: Deserializer<'de>>(
    deserializer: CountryDeserializer,
) -> Result<Option<String>, CountryDeserializer::Error> {
    let code = Option::<String>::deserialize(deserializer)?;
    Ok(code.and_then(|code| {
        let country = chart_country(&code).map(|country| country.code.to_owned());
        if country.is_none() {
            tracing::warn!("ignoring the unknown chart country {code}");
        }
        country
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> YouTubeSettings {
        toml::from_str(text).unwrap()
    }

    #[test]
    fn defaults_use_yt_dlp_from_the_path_without_cookies() {
        let settings = YouTubeSettings::default();
        assert_eq!(settings.yt_dlp_program(), "yt-dlp");
        assert_eq!(settings.cookies_from_browser, None);
    }

    #[test]
    fn a_blank_path_falls_back_to_the_default_program() {
        let settings = parse("yt_dlp_path = \"  \"");
        assert_eq!(settings.yt_dlp_program(), "yt-dlp");
        let custom = parse("yt_dlp_path = \"/opt/yt-dlp\"");
        assert_eq!(custom.yt_dlp_program(), "/opt/yt-dlp");
    }

    #[test]
    fn browsers_are_read_by_name_and_unknown_ones_are_ignored() {
        let firefox = parse("cookies_from_browser = \"firefox\"");
        assert_eq!(firefox.cookies_from_browser, Some(CookieBrowser::Firefox));
        let unknown = parse("cookies_from_browser = \"netscape\"");
        assert_eq!(unknown.cookies_from_browser, None);
    }

    #[test]
    fn countries_are_read_by_code_and_unknown_ones_are_ignored() {
        assert_eq!(parse("country = \"us\"").country.as_deref(), Some("US"));
        assert_eq!(parse("country = \"XX\"").country, None);
        assert_eq!(YouTubeSettings::default().country, None);
    }

    #[test]
    fn settings_round_trip_through_toml() {
        let settings = YouTubeSettings {
            yt_dlp_path: Some("/usr/local/bin/yt-dlp".into()),
            cookies_from_browser: Some(CookieBrowser::Vivaldi),
            country: Some("DE".into()),
        };
        let text = toml::to_string(&settings).unwrap();
        assert!(text.contains("cookies_from_browser = \"vivaldi\""));
        assert_eq!(parse(&text), settings);
    }
}
