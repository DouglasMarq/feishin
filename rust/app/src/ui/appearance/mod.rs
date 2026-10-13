use feishin_platform::SystemColorScheme;
use feishin_storage::appearance::{AccentColor, AppearanceSettings};
use slint::{ComponentHandle, SharedString};

use super::{AppWindow, AppearanceState, Context, on_ui, show_status};
use crate::controller::Command;

mod apply;
mod catalog;
mod form;
mod resolve;

#[cfg(test)]
mod tests;

use apply::apply_theme;
use catalog::PaletteCatalog;

pub fn install(window: &AppWindow, ctx: &Context, settings: &AppearanceSettings) {
    let catalog = PaletteCatalog::from_window(window);
    form::show(window, &catalog, settings);
    apply_theme(window, &catalog, settings);
    install_callbacks(window, ctx);
}

fn install_callbacks(window: &AppWindow, ctx: &Context) {
    let state = window.global::<AppearanceState>();
    let weak_window = window.as_weak();
    let changed_ctx = ctx.clone();
    state.on_changed(move || {
        if let Some(window) = weak_window.upgrade() {
            save_changes(&window, &changed_ctx);
        }
    });
    let weak_window = window.as_weak();
    let entered_ctx = ctx.clone();
    state.on_accent_entered(move |text| {
        if let Some(window) = weak_window.upgrade() {
            enter_accent(&window, &entered_ctx, &text);
        }
    });
}

fn current_settings(window: &AppWindow) -> (PaletteCatalog, AppearanceSettings) {
    let catalog = PaletteCatalog::from_window(window);
    let settings = form::read(window, &catalog);
    (catalog, settings)
}

fn save_changes(window: &AppWindow, ctx: &Context) {
    let (catalog, settings) = current_settings(window);
    form::show_accent(&window.global::<AppearanceState>(), settings.accent);
    apply_theme(window, &catalog, &settings);
    ctx.send(Command::SaveAppearance(settings));
}

fn enter_accent(window: &AppWindow, ctx: &Context, text: &SharedString) {
    let state = window.global::<AppearanceState>();
    match text.parse::<AccentColor>() {
        Ok(accent) => {
            state.set_accent(form::slint_color(accent));
            save_changes(window, ctx);
        }
        Err(error) => {
            let (_, settings) = current_settings(window);
            form::show_accent(&state, settings.accent);
            show_status(window, &format!("Accent color not changed: {error}."));
        }
    }
}

fn system_scheme_changed(window: &AppWindow, scheme: SystemColorScheme) {
    window
        .global::<AppearanceState>()
        .set_system_scheme(form::slint_system_scheme(scheme));
    let (catalog, settings) = current_settings(window);
    apply_theme(window, &catalog, &settings);
}

pub fn watch_system_scheme(window: &AppWindow, ctx: &Context) {
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
