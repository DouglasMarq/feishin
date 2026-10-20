use feishin_platform::SystemColorScheme;
use feishin_storage::appearance::{AccentColor, AppearanceSettings, ThemeId};
use slint::{Color, ComponentHandle};

use super::catalog::PaletteCatalog;
use crate::ui::{AppWindow, AppearanceState, SystemScheme, slint_index};

pub(super) fn slint_color(accent: AccentColor) -> Color {
    Color::from_rgb_u8(accent.red, accent.green, accent.blue)
}

fn accent_color(color: Color) -> AccentColor {
    AccentColor::new(color.red(), color.green(), color.blue())
}

pub(super) fn system_scheme(state: &AppearanceState) -> SystemColorScheme {
    match state.get_system_scheme() {
        SystemScheme::Dark => SystemColorScheme::Dark,
        SystemScheme::Light => SystemColorScheme::Light,
        SystemScheme::NoPreference => SystemColorScheme::NoPreference,
    }
}

pub(super) fn slint_system_scheme(scheme: SystemColorScheme) -> SystemScheme {
    match scheme {
        SystemColorScheme::Dark => SystemScheme::Dark,
        SystemColorScheme::Light => SystemScheme::Light,
        SystemColorScheme::NoPreference => SystemScheme::NoPreference,
    }
}

pub(super) fn show(window: &AppWindow, catalog: &PaletteCatalog, settings: &AppearanceSettings) {
    let state = window.global::<AppearanceState>();
    let dark_default = ThemeId::default_dark();
    let light_default = ThemeId::default_light();
    state.set_follow_system(settings.follow_system);
    state.set_theme_index(slint_index(
        catalog.index_or(&settings.theme, &dark_default),
    ));
    state.set_dark_theme_index(slint_index(
        catalog.index_or(&settings.dark_theme, &dark_default),
    ));
    state.set_light_theme_index(slint_index(
        catalog.index_or(&settings.light_theme, &light_default),
    ));
    state.set_use_theme_accent(settings.use_theme_accent);
    show_accent(&state, settings.accent);
}

pub(super) fn show_accent(state: &AppearanceState, accent: AccentColor) {
    state.set_accent(slint_color(accent));
    state.set_accent_text(accent.to_string().into());
}

pub(super) fn read(window: &AppWindow, catalog: &PaletteCatalog) -> AppearanceSettings {
    let state = window.global::<AppearanceState>();
    AppearanceSettings {
        theme: catalog
            .id_at(state.get_theme_index())
            .unwrap_or_else(ThemeId::default_dark),
        follow_system: state.get_follow_system(),
        dark_theme: catalog
            .id_at(state.get_dark_theme_index())
            .unwrap_or_else(ThemeId::default_dark),
        light_theme: catalog
            .id_at(state.get_light_theme_index())
            .unwrap_or_else(ThemeId::default_light),
        use_theme_accent: state.get_use_theme_accent(),
        accent: accent_color(state.get_accent()),
    }
}
