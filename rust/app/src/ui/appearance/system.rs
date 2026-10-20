use std::time::Duration;

use feishin_platform::SystemColorScheme;
use slint::ComponentHandle;

use super::apply::apply_theme;
use super::{current_settings, form};
use crate::ui::{AppWindow, AppearanceState, Context, on_ui};

const STARTUP_SCHEME_TIMEOUT: Duration = Duration::from_millis(300);

pub(super) fn system_scheme_changed(window: &AppWindow, scheme: SystemColorScheme) {
    window
        .global::<AppearanceState>()
        .set_system_scheme(form::slint_system_scheme(scheme));
    let (catalog, settings) = current_settings(window);
    apply_theme(window, &catalog, &settings);
}

fn apply_startup_scheme(window: &AppWindow, ctx: &Context) {
    let weak_window = window.as_weak();
    ctx.rt.spawn(async move {
        let read = tokio::time::timeout(
            STARTUP_SCHEME_TIMEOUT,
            feishin_platform::current_color_scheme(),
        )
        .await;
        match read {
            Ok(Ok(scheme)) => on_ui(&weak_window, move |window| {
                system_scheme_changed(window, scheme);
            }),
            Ok(Err(error)) => tracing::debug!("could not read the desktop color scheme: {error}"),
            Err(_) => tracing::debug!("the desktop color scheme took too long to read"),
        }
    });
}

pub fn watch_system_scheme(window: &AppWindow, ctx: &Context) {
    if window.global::<AppearanceState>().get_follow_system() {
        apply_startup_scheme(window, ctx);
    }
    let weak_window = window.as_weak();
    ctx.rt.spawn(async move {
        let watched = feishin_platform::watch_color_scheme(|scheme| {
            on_ui(&weak_window, move |window| {
                system_scheme_changed(window, scheme);
            });
        })
        .await;
        if let Err(error) = watched {
            tracing::debug!("not following the desktop color scheme: {error}");
        }
    });
}
