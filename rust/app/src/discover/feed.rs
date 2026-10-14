use feishin_storage::settings::YouTubeSettings;
use feishin_youtube::CookieBrowser;
use feishin_youtube::music::{
    GLOBAL_CHARTS, MoodCategory, MusicLocale, MusicShelf, SystemLocale, chart_country_or_fallback,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FeedRequest {
    ForYou,
    NewReleases,
    Charts { country: String },
    Moods,
    Mood { params: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum FeedNotice {
    #[default]
    None,
    Demo,
    SignInHint,
    Personalized(CookieBrowser),
    SignInFailed(String),
}

impl FeedNotice {
    pub fn message(&self) -> String {
        match self {
            Self::None => String::new(),
            Self::Demo => tr!("Demo mode: these picks are made up and play from your demo library"),
            Self::SignInHint => {
                tr!("Choose a browser under Settings, YouTube to sign in for a personalized home")
            }
            Self::Personalized(browser) => tr!(
                "Personalized with your YouTube Music account from {}",
                browser.label()
            ),
            Self::SignInFailed(reason) => tr!(
                "Could not sign in to YouTube Music ({}). Showing general picks instead.",
                reason
            ),
        }
    }

    pub fn is_warning(&self) -> bool {
        matches!(self, Self::SignInFailed(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiscoverFeed {
    pub shelves: Vec<MusicShelf>,
    pub moods: Vec<MoodCategory>,
    pub notice: FeedNotice,
}

impl DiscoverFeed {
    pub fn of_shelves(shelves: Vec<MusicShelf>, notice: FeedNotice) -> Self {
        Self {
            shelves,
            moods: Vec::new(),
            notice,
        }
    }
}

pub fn default_chart_country(settings: &YouTubeSettings, system: &SystemLocale) -> String {
    chart_country_or_fallback(&[settings.country.as_deref(), system.region.as_deref()])
        .code
        .to_owned()
}

pub fn music_locale(settings: &YouTubeSettings, system: &SystemLocale) -> MusicLocale {
    let regional_country = settings
        .country
        .as_deref()
        .filter(|country| *country != GLOBAL_CHARTS);
    MusicLocale {
        language: system.language_or_default(),
        country: chart_country_or_fallback(&[regional_country, system.region.as_deref()])
            .code
            .to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings_with_country(country: Option<&str>) -> YouTubeSettings {
        YouTubeSettings {
            country: country.map(str::to_owned),
            ..YouTubeSettings::default()
        }
    }

    #[test]
    fn the_chart_country_comes_from_settings_then_the_system_then_brazil() {
        let german = SystemLocale::parse("de_DE.UTF-8");
        assert_eq!(
            default_chart_country(&settings_with_country(Some("US")), &german),
            "US"
        );
        assert_eq!(
            default_chart_country(&settings_with_country(None), &german),
            "DE"
        );
        assert_eq!(
            default_chart_country(&settings_with_country(None), &SystemLocale::default()),
            "BR"
        );
    }

    #[test]
    fn requests_use_the_system_language_and_a_real_country() {
        let locale = music_locale(
            &settings_with_country(Some("ZZ")),
            &SystemLocale::parse("fr_CA.UTF-8"),
        );
        assert_eq!(locale.language, "fr");
        assert_eq!(locale.country, "CA");
        let fallback = music_locale(&settings_with_country(None), &SystemLocale::default());
        assert_eq!(
            (fallback.language.as_str(), fallback.country.as_str()),
            ("en", "BR")
        );
    }

    #[test]
    fn only_sign_in_failures_are_warnings() {
        assert!(FeedNotice::SignInFailed("x".into()).is_warning());
        assert!(!FeedNotice::Personalized(CookieBrowser::Firefox).is_warning());
        assert!(FeedNotice::None.message().is_empty());
    }
}
