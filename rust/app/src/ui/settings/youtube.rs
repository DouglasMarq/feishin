use std::sync::Arc;

use feishin_storage::settings::YouTubeSettings;
use feishin_youtube::CookieBrowser;
use feishin_youtube::music::{CHART_COUNTRIES, SystemLocale, chart_country_or_fallback};
use slint::ComponentHandle;

use super::choices::Choices;
use super::display::{slint_index, string_model};
use super::view::{Shared, View};
use crate::controller::{Command, YouTubeCommand};
use crate::ui::{AppWindow, Context, SettingsState};

pub(super) fn cookie_browser_choices(
    current: Option<CookieBrowser>,
) -> Choices<Option<CookieBrowser>> {
    let options = std::iter::once((tr!("None"), None))
        .chain(
            CookieBrowser::ALL
                .into_iter()
                .map(|browser| (browser.label().to_owned(), Some(browser))),
        )
        .collect();
    Choices::new(options, &current)
}

pub(super) fn chart_country_choices(
    current: Option<&str>,
    system: &SystemLocale,
) -> Choices<Option<String>> {
    let automatic = chart_country_or_fallback(&[system.region.as_deref()]);
    let options = std::iter::once((tr!("Automatic ({})", automatic.name), None))
        .chain(
            CHART_COUNTRIES
                .iter()
                .map(|country| (country.name.to_owned(), Some(country.code.to_owned()))),
        )
        .collect();
    Choices::new(options, &current.map(str::to_owned))
}

pub(super) fn show_youtube(state: &SettingsState, view: &View) {
    let youtube = &view.settings.youtube;
    state.set_yt_dlp_path(youtube.yt_dlp_path.as_deref().unwrap_or_default().into());
    state.set_cookie_browsers(string_model(&view.cookie_browsers.labels));
    state.set_cookie_browser_index(slint_index(view.cookie_browsers.selected));
    state.set_chart_countries(string_model(&view.chart_countries.labels));
    state.set_chart_country_index(slint_index(view.chart_countries.selected));
}

pub(super) fn youtube_form(state: &SettingsState, view: &View) -> YouTubeSettings {
    let path = state.get_yt_dlp_path().trim().to_owned();
    let cookies_from_browser = view
        .cookie_browsers
        .value(state.get_cookie_browser_index())
        .unwrap_or(view.settings.youtube.cookies_from_browser);
    let country = view
        .chart_countries
        .value(state.get_chart_country_index())
        .unwrap_or_else(|| view.settings.youtube.country.clone());
    YouTubeSettings {
        yt_dlp_path: Some(path).filter(|path| !path.is_empty()),
        cookies_from_browser,
        country,
    }
}

pub(super) fn apply_youtube(window: &AppWindow, ctx: &Context, shared: &Arc<Shared>) {
    let state = window.global::<SettingsState>();
    let mut view = shared.view();
    let youtube = youtube_form(&state, &view);
    if youtube == view.settings.youtube {
        return;
    }
    view.settings.youtube = youtube.clone();
    ctx.send(Command::YouTube(YouTubeCommand::SaveSettings(youtube)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browsers_start_with_none_and_select_the_saved_one() {
        let choices = cookie_browser_choices(None);
        assert_eq!(choices.labels.len(), 9);
        assert_eq!(choices.labels[0], "None");
        assert_eq!(choices.selected, 0);
        let firefox = cookie_browser_choices(Some(CookieBrowser::Firefox));
        assert_eq!(firefox.labels[firefox.selected], "Firefox");
        assert_eq!(firefox.value(8), Some(Some(CookieBrowser::Whale)));
        assert_eq!(firefox.value(0), Some(None));
    }

    #[test]
    fn chart_countries_start_with_automatic_from_the_system_region() {
        let german = SystemLocale::parse("de_DE.UTF-8");
        let choices = chart_country_choices(None, &german);
        assert_eq!(choices.labels[0], "Automatic (Germany)");
        assert_eq!(choices.selected, 0);
        let unknown = chart_country_choices(None, &SystemLocale::default());
        assert_eq!(unknown.labels[0], "Automatic (Brazil)");
        let saved = chart_country_choices(Some("US"), &german);
        assert_eq!(saved.labels[saved.selected], "United States");
        assert_eq!(saved.value(1), Some(Some("ZZ".to_owned())));
    }
}
