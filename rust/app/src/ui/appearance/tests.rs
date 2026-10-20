use std::sync::Arc;

use feishin_platform::SystemColorScheme;
use feishin_storage::appearance::{AccentColor, AppearanceSettings, ThemeId};
use slint::{Color, ComponentHandle};
use tokio::sync::mpsc::UnboundedReceiver;

use super::catalog::PaletteCatalog;
use super::install;
use super::system::system_scheme_changed;
use crate::connect::DemoConnector;
use crate::controller::Command;
use crate::covers::DemoCovers;
use crate::session::Session;
use crate::store::JsonStore;
use crate::ui::{AppState, AppWindow, AppearanceState, Context, Theme};

const ELECTRON_THEME_IDS: [&str; 32] = [
    "ayuDark",
    "ayuLight",
    "catppuccinLatte",
    "catppuccinMocha",
    "defaultDark",
    "defaultLight",
    "dracula",
    "everforestDark",
    "everforestLight",
    "githubDark",
    "githubLight",
    "glassyDark",
    "gruvboxDark",
    "gruvboxLight",
    "highContrastDark",
    "highContrastLight",
    "materialDark",
    "materialLight",
    "monokai",
    "nightOwl",
    "nord",
    "oneDark",
    "rosePine",
    "rosePineDawn",
    "rosePineMoon",
    "shadesOfPurple",
    "solarizedDark",
    "solarizedLight",
    "tokyoNight",
    "vscodeDarkPlus",
    "vscodeLightPlus",
    "zenburn",
];

struct AppearancePage {
    window: AppWindow,
    received: UnboundedReceiver<Command>,
    _runtime: tokio::runtime::Runtime,
    _dir: tempfile::TempDir,
}

impl AppearancePage {
    fn theme(&self) -> Theme<'_> {
        self.window.global::<Theme>()
    }

    fn state(&self) -> AppearanceState<'_> {
        self.window.global::<AppearanceState>()
    }

    fn saved(&mut self) -> Vec<AppearanceSettings> {
        std::iter::from_fn(|| self.received.try_recv().ok())
            .filter_map(|command| match command {
                Command::SaveAppearance(settings) => Some(settings),
                _ => None,
            })
            .collect()
    }
}

fn open_with(settings: &AppearanceSettings) -> AppearancePage {
    i_slint_backend_testing::init_no_event_loop();
    let window = AppWindow::new().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let (commands, received) = tokio::sync::mpsc::unbounded_channel();
    let context = Context {
        runtime: runtime.handle().clone(),
        commands,
        session: Session::default(),
        store: Arc::new(JsonStore::new(dir.path())),
        connector: Arc::new(DemoConnector::default()),
        covers: Arc::new(DemoCovers),
        remote: crate::remote::Remote::default(),
    };
    install(&window, &context, settings);
    AppearancePage {
        window,
        received,
        _runtime: runtime,
        _dir: dir,
    }
}

fn themed(theme: &str) -> AppearanceSettings {
    AppearanceSettings {
        theme: ThemeId::new(theme),
        ..AppearanceSettings::default()
    }
}

#[test]
fn every_electron_theme_has_a_palette() {
    let page = open_with(&AppearanceSettings::default());
    let catalog = PaletteCatalog::from_window(&page.window);
    let mut ids: Vec<&str> = catalog.ids().map(ThemeId::as_str).collect();
    assert_eq!(ids.first(), Some(&"defaultDark"));
    ids.sort_unstable();
    assert_eq!(ids, ELECTRON_THEME_IDS);
}

#[test]
fn the_default_theme_keeps_the_original_look() {
    let page = open_with(&AppearanceSettings::default());
    let theme = page.theme();
    assert!(theme.get_dark());
    assert_eq!(theme.get_background(), Color::from_rgb_u8(0x0c, 0x0c, 0x0c));
    assert_eq!(theme.get_background_alt(), Color::from_rgb_u8(8, 8, 8));
    assert_eq!(theme.get_surface(), Color::from_rgb_u8(0x14, 0x14, 0x14));
    assert_eq!(
        theme.get_surface_raised(),
        Color::from_rgb_u8(0x1d, 0x1d, 0x1d)
    );
    assert_eq!(theme.get_border(), Color::from_rgb_u8(0x23, 0x23, 0x23));
    assert_eq!(theme.get_foreground(), Color::from_rgb_u8(0xe1, 0xe1, 0xe1));
    assert_eq!(theme.get_muted(), Color::from_rgb_u8(0x96, 0x96, 0x96));
    assert_eq!(theme.get_primary(), Color::from_rgb_u8(0x35, 0x74, 0xfc));
    assert_eq!(theme.get_danger(), Color::from_rgb_u8(0xf2, 0x57, 0x57));
    assert_eq!(
        theme.get_hover(),
        Color::from_argb_u8(0x0f, 0xff, 0xff, 0xff)
    );
    assert_eq!(
        theme.get_selected(),
        Color::from_argb_u8(0x29, 0x35, 0x74, 0xfc)
    );
    assert_eq!(theme.get_on_primary(), Color::from_rgb_u8(0xff, 0xff, 0xff));
}

#[test]
fn a_saved_light_theme_is_applied_on_start() {
    let page = open_with(&themed("githubLight"));
    let theme = page.theme();
    assert!(!theme.get_dark());
    assert_eq!(theme.get_background(), Color::from_rgb_u8(255, 255, 255));
    assert_eq!(theme.get_hover(), Color::from_argb_u8(0x0f, 0, 0, 0));
}

#[test]
fn an_unknown_theme_falls_back_to_the_default() {
    let page = open_with(&themed("someFutureTheme"));
    assert_eq!(page.state().get_theme_index(), 0);
    assert_eq!(
        page.theme().get_background(),
        Color::from_rgb_u8(0x0c, 0x0c, 0x0c)
    );
}

#[test]
fn picking_a_theme_applies_it_live_and_saves_it() {
    let mut page = open_with(&AppearanceSettings::default());
    let catalog = PaletteCatalog::from_window(&page.window);
    let nord = catalog.index_of(&ThemeId::new("nord")).unwrap();
    page.state().set_theme_index(i32::try_from(nord).unwrap());
    page.state().invoke_changed();
    assert_eq!(
        page.theme().get_background(),
        Color::from_rgb_u8(46, 52, 64)
    );
    let saved = page.saved();
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].theme, ThemeId::new("nord"));
    assert!(!saved[0].is_following_system);
}

#[test]
fn following_the_system_tracks_the_desktop_scheme() {
    let settings = AppearanceSettings {
        is_following_system: true,
        dark_theme: ThemeId::new("dracula"),
        light_theme: ThemeId::new("solarizedLight"),
        ..AppearanceSettings::default()
    };
    let page = open_with(&settings);
    assert_eq!(
        page.theme().get_background(),
        Color::from_rgb_u8(40, 42, 54)
    );
    system_scheme_changed(&page.window, SystemColorScheme::Light);
    assert_eq!(
        page.theme().get_background(),
        Color::from_rgb_u8(253, 246, 227)
    );
    assert!(!page.theme().get_dark());
    system_scheme_changed(&page.window, SystemColorScheme::Dark);
    assert_eq!(
        page.theme().get_background(),
        Color::from_rgb_u8(40, 42, 54)
    );
}

#[test]
fn a_custom_accent_replaces_the_theme_primary() {
    let mut page = open_with(&AppearanceSettings::default());
    page.state().set_use_theme_accent(false);
    page.state().invoke_accent_entered("#1db954".into());
    let theme = page.theme();
    assert_eq!(theme.get_primary(), Color::from_rgb_u8(0x1d, 0xb9, 0x54));
    assert_eq!(
        theme.get_selected(),
        Color::from_argb_u8(0x29, 0x1d, 0xb9, 0x54)
    );
    assert_eq!(page.state().get_accent_text(), "#1db954");
    let saved = page.saved();
    assert_eq!(saved.len(), 1);
    assert!(!saved[0].is_using_theme_accent);
    assert_eq!(saved[0].accent, AccentColor::new(0x1d, 0xb9, 0x54));
}

#[test]
fn a_malformed_accent_is_reported_and_reverted() {
    let mut page = open_with(&AppearanceSettings::default());
    page.state().set_use_theme_accent(false);
    page.state().invoke_accent_entered("#nothex".into());
    assert_eq!(page.state().get_accent_text(), "#3574fc");
    assert!(
        page.window
            .global::<AppState>()
            .get_status()
            .contains("Accent color")
    );
    assert!(page.saved().is_empty());
}

#[test]
fn light_accents_get_dark_text() {
    let page = open_with(&themed("tokyoNight"));
    assert_eq!(
        page.theme().get_on_primary(),
        Color::from_rgb_u8(0x14, 0x14, 0x14)
    );
}

#[cfg(debug_assertions)]
mod clicks;
