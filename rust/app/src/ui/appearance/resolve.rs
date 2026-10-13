use feishin_platform::SystemColorScheme;
use feishin_storage::appearance::{AppearanceSettings, ThemeId};

pub(super) fn active_theme(
    settings: &AppearanceSettings,
    system: SystemColorScheme,
) -> (&ThemeId, ThemeId) {
    match (settings.follow_system, system) {
        (false, _) => (&settings.theme, ThemeId::default_dark()),
        (true, SystemColorScheme::Light) => (&settings.light_theme, ThemeId::default_light()),
        (true, SystemColorScheme::Dark | SystemColorScheme::NoPreference) => {
            (&settings.dark_theme, ThemeId::default_dark())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn following_system() -> AppearanceSettings {
        AppearanceSettings {
            theme: ThemeId::new("nord"),
            follow_system: true,
            dark_theme: ThemeId::new("dracula"),
            light_theme: ThemeId::new("githubLight"),
            ..AppearanceSettings::default()
        }
    }

    #[test]
    fn the_chosen_theme_wins_when_not_following_the_system() {
        let settings = AppearanceSettings {
            follow_system: false,
            ..following_system()
        };
        for system in [
            SystemColorScheme::Dark,
            SystemColorScheme::Light,
            SystemColorScheme::NoPreference,
        ] {
            assert_eq!(active_theme(&settings, system).0, &ThemeId::new("nord"));
        }
    }

    #[test]
    fn following_the_system_picks_the_matching_theme() {
        let settings = following_system();
        assert_eq!(
            active_theme(&settings, SystemColorScheme::Dark),
            (&ThemeId::new("dracula"), ThemeId::default_dark())
        );
        assert_eq!(
            active_theme(&settings, SystemColorScheme::Light),
            (&ThemeId::new("githubLight"), ThemeId::default_light())
        );
    }

    #[test]
    fn no_system_preference_falls_back_to_the_dark_theme() {
        let settings = following_system();
        assert_eq!(
            active_theme(&settings, SystemColorScheme::NoPreference).0,
            &ThemeId::new("dracula")
        );
    }
}
