use std::sync::Arc;

use slint::ComponentHandle;

use super::display::string_model;
use super::loading::load;
use super::view::Shared;
use crate::controller::Command;
use crate::i18n::{self, LANGUAGES};
use crate::ui::{AppWindow, Context, SettingsState, slint_index};

fn language_labels() -> Vec<String> {
    std::iter::once(tr!("System default"))
        .chain(LANGUAGES.iter().map(|language| language.name.to_owned()))
        .collect()
}

pub(super) fn language_index(preference: Option<&str>) -> usize {
    preference
        .and_then(|code| LANGUAGES.iter().position(|language| language.code == code))
        .map_or(0, |position| position + 1)
}

pub(super) fn language_at(index: i32) -> Option<String> {
    let position = usize::try_from(index).ok()?.checked_sub(1)?;
    LANGUAGES
        .get(position)
        .map(|language| language.code.to_owned())
}

pub(super) fn show_languages(state: &SettingsState, preference: Option<&str>) {
    state.set_languages(string_model(&language_labels()));
    state.set_language_index(slint_index(language_index(preference)));
}

pub(super) fn apply_language(window: &AppWindow, context: &Context, shared: &Arc<Shared>) {
    let state = window.global::<SettingsState>();
    let language = language_at(state.get_language_index());
    i18n::select_language(language.as_deref());
    show_languages(&state, language.as_deref());
    shared.view().settings.ui.language.clone_from(&language);
    context.send(Command::SaveLanguage(language));
    crate::ui::albums::show_sort_options(window);
    crate::ui::artists::show_sort_options(window);
    load(window, context, shared);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_choice_follows_the_system() {
        assert_eq!(language_index(None), 0);
        assert_eq!(language_at(0), None);
    }

    #[test]
    fn choices_round_trip_to_language_codes() {
        let index = language_index(Some("pt-BR"));
        assert_eq!(language_at(slint_index(index)).as_deref(), Some("pt-BR"));
        assert_eq!(
            language_labels().get(index).map(String::as_str),
            Some("Português (Brasil)")
        );
    }

    #[test]
    fn unknown_codes_and_indexes_fall_back_to_the_system() {
        assert_eq!(language_index(Some("xx")), 0);
        assert_eq!(language_at(-1), None);
        assert_eq!(language_at(500), None);
    }
}
