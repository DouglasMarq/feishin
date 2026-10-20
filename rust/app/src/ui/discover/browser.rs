use slint::ComponentHandle;

use super::deps::DiscoverDeps;
use crate::ui::{AppWindow, on_ui, show_status};

pub fn open_in_browser(window: &AppWindow, deps: &DiscoverDeps, url: String) {
    let weak_window = window.as_weak();
    deps.context.runtime.spawn_blocking(move || {
        let Err(error) = feishin_platform::open_url(&url) else {
            return;
        };
        tracing::warn!("opening {url} failed: {error}");
        on_ui(&weak_window, move |window| {
            show_status(window, &tr!("Could not open YouTube Music: {}", error));
        });
    });
}
