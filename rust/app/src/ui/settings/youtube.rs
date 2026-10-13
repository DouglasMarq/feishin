use std::sync::Arc;

use feishin_storage::settings::YouTubeSettings;
use feishin_youtube::CookieBrowser;
use slint::ComponentHandle;

use super::choices::Choices;
use super::display::{slint_index, string_model};
use super::view::{Shared, View};
use crate::controller::{Command, YouTubeCommand};
use crate::ui::{AppWindow, Context, SettingsState};

const NO_BROWSER_LABEL: &str = "None";

pub(super) fn cookie_browser_choices(
    current: Option<CookieBrowser>,
) -> Choices<Option<CookieBrowser>> {
    let options = std::iter::once((NO_BROWSER_LABEL.to_owned(), None))
        .chain(
            CookieBrowser::ALL
                .into_iter()
                .map(|browser| (browser.label().to_owned(), Some(browser))),
        )
        .collect();
    Choices::new(options, &current)
}

pub(super) fn show_youtube(state: &SettingsState, view: &View) {
    let youtube = &view.settings.youtube;
    state.set_yt_dlp_path(youtube.yt_dlp_path.as_deref().unwrap_or_default().into());
    state.set_cookie_browsers(string_model(&view.cookie_browsers.labels));
    state.set_cookie_browser_index(slint_index(view.cookie_browsers.selected));
}

pub(super) fn youtube_form(state: &SettingsState, view: &View) -> YouTubeSettings {
    let path = state.get_yt_dlp_path().trim().to_owned();
    let cookies_from_browser = view
        .cookie_browsers
        .value(state.get_cookie_browser_index())
        .unwrap_or(view.settings.youtube.cookies_from_browser);
    YouTubeSettings {
        yt_dlp_path: Some(path).filter(|path| !path.is_empty()),
        cookies_from_browser,
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
}
