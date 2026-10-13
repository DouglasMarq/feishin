use feishin_storage::appearance::AppearanceSettings;
use slint::ComponentHandle;

use super::catalog::PaletteCatalog;
use super::form::{slint_color, system_scheme};
use super::resolve::active_theme;
use crate::ui::{AppWindow, AppearanceState, Theme};

pub(super) fn apply_theme(
    window: &AppWindow,
    catalog: &PaletteCatalog,
    settings: &AppearanceSettings,
) {
    let system = system_scheme(&window.global::<AppearanceState>());
    let (theme, fallback) = active_theme(settings, system);
    let index = catalog.index_or(theme, &fallback);
    window.global::<Theme>().invoke_apply(
        i32::try_from(index).unwrap_or_default(),
        !settings.use_theme_accent,
        slint_color(settings.accent),
    );
}
