use feishin_youtube::music::CHART_COUNTRIES;
use slint::ComponentHandle;

use super::card_actions::{open_card_in_browser, place_card, play_from_card};
use super::covers::{card_visibility_changed, load_card_cover, load_song_cover, song_rows_shown};
use super::deps::DiscoverDeps;
use super::loading::{refresh_sections, show_sections};
use super::model::with_discover;
use super::page::search_detail_id;
use super::page_actions::{
    open_page_in_browser, open_song_in_browser, place_page_song, play_page_from, queue_page,
};
use super::page_loading::open_page;
use super::playing::Placement;
use super::sections::Section;
use crate::ui::{AppState, AppWindow, DiscoverState, Route};

type WindowAction = fn(&AppWindow, &DiscoverDeps);
type IndexAction = fn(&AppWindow, &DiscoverDeps, usize);
type CardAction = fn(&AppWindow, &DiscoverDeps, usize, usize);

fn on_window(window: &AppWindow, deps: &DiscoverDeps, action: WindowAction) -> impl Fn() + 'static {
    let (weak_window, deps) = (window.as_weak(), deps.clone());
    move || {
        if let Some(window) = weak_window.upgrade() {
            action(&window, &deps);
        }
    }
}

fn on_index(
    window: &AppWindow,
    deps: &DiscoverDeps,
    action: IndexAction,
) -> impl Fn(i32) + 'static {
    let (weak_window, deps) = (window.as_weak(), deps.clone());
    move |index| {
        if let (Some(window), Ok(index)) = (weak_window.upgrade(), usize::try_from(index)) {
            action(&window, &deps, index);
        }
    }
}

fn on_card(
    window: &AppWindow,
    deps: &DiscoverDeps,
    action: CardAction,
) -> impl Fn(i32, i32) + 'static {
    let (weak_window, deps) = (window.as_weak(), deps.clone());
    move |shelf, card| {
        if let (Some(window), Ok(shelf), Ok(card)) = (
            weak_window.upgrade(),
            usize::try_from(shelf),
            usize::try_from(card),
        ) {
            action(&window, &deps, shelf, card);
        }
    }
}

fn select_country(window: &AppWindow, deps: &DiscoverDeps, index: usize) {
    let Some(country) = CHART_COUNTRIES.get(index) else {
        return;
    };
    with_discover(window, |model| {
        model.sections.borrow_mut().chart_country = Some(country.code.to_owned());
    });
    show_sections(window, deps);
}

fn select_mood(window: &AppWindow, deps: &DiscoverDeps, index: usize) {
    with_discover(window, |model| {
        let mut sections = model.sections.borrow_mut();
        if index < sections.moods().len() {
            sections.mood = Some(index);
        }
    });
    show_sections(window, deps);
}

fn install_navigation(window: &AppWindow, deps: &DiscoverDeps) {
    let state = window.global::<DiscoverState>();
    let (weak_window, open_deps) = (window.as_weak(), deps.clone());
    state.on_open(move |detail_id| {
        let Some(window) = weak_window.upgrade() else {
            return;
        };
        match detail_id.is_empty() {
            true => show_sections(&window, &open_deps),
            false => open_page(&window, &open_deps, &detail_id),
        }
    });
    let (weak_window, section_deps) = (window.as_weak(), deps.clone());
    state.on_select_section(move |section| {
        let Some(window) = weak_window.upgrade() else {
            return;
        };
        with_discover(&window, |model| {
            model.sections.borrow_mut().section = Section::from_slint(section);
        });
        show_sections(&window, &section_deps);
    });
    let weak_window = window.as_weak();
    state.on_search(move |query| {
        if let (Some(window), Some(detail_id)) = (weak_window.upgrade(), search_detail_id(&query)) {
            window
                .global::<AppState>()
                .invoke_navigate(Route::Discover, detail_id.into());
        }
    });
    state.on_refresh(on_window(window, deps, refresh_sections));
    state.on_select_country(on_index(window, deps, select_country));
    state.on_select_mood(on_index(window, deps, select_mood));
}

fn install_visibility(window: &AppWindow) {
    let state = window.global::<DiscoverState>();
    let weak_window = window.as_weak();
    state.on_card_visibility_changed(move |shelf, card, visible| {
        if let (Some(window), Ok(shelf), Ok(card)) = (
            weak_window.upgrade(),
            usize::try_from(shelf),
            usize::try_from(card),
        ) {
            card_visibility_changed(&window, shelf, card, visible);
        }
    });
    let weak_window = window.as_weak();
    state.on_song_rows_shown(move |first_row, row_count| {
        if let (Some(window), Ok(first_row), Ok(row_count)) = (
            weak_window.upgrade(),
            usize::try_from(first_row),
            usize::try_from(row_count),
        ) {
            song_rows_shown(&window, first_row, row_count);
        }
    });
}

fn install_cards(window: &AppWindow, deps: &DiscoverDeps) {
    let state = window.global::<DiscoverState>();
    state.on_cover_needed(on_card(window, deps, load_card_cover));
    state.on_play_from_card(on_card(window, deps, play_from_card));
    state.on_play_card(on_card(window, deps, |window, deps, shelf, card| {
        place_card(window, deps, (shelf, card), Placement::PlayFrom(0));
    }));
    state.on_play_card_next(on_card(window, deps, |window, deps, shelf, card| {
        place_card(window, deps, (shelf, card), Placement::Next);
    }));
    state.on_queue_card(on_card(window, deps, |window, deps, shelf, card| {
        place_card(window, deps, (shelf, card), Placement::Last);
    }));
    state.on_open_card_in_browser(on_card(window, deps, open_card_in_browser));
}

fn install_page(window: &AppWindow, deps: &DiscoverDeps) {
    let state = window.global::<DiscoverState>();
    state.on_song_cover_needed(on_index(window, deps, load_song_cover));
    state.on_play_song(on_index(window, deps, play_page_from));
    state.on_play_song_next(on_index(window, deps, |window, deps, index| {
        place_page_song(window, deps, index, Placement::Next);
    }));
    state.on_queue_song(on_index(window, deps, |window, deps, index| {
        place_page_song(window, deps, index, Placement::Last);
    }));
    state.on_open_song_in_browser(on_index(window, deps, open_song_in_browser));
    state.on_play_page(on_window(window, deps, |window, deps| {
        play_page_from(window, deps, 0)
    }));
    state.on_queue_page(on_window(window, deps, queue_page));
    state.on_open_page_in_browser(on_window(window, deps, open_page_in_browser));
}

pub fn install_callbacks(window: &AppWindow, deps: &DiscoverDeps) {
    install_navigation(window, deps);
    install_cards(window, deps);
    install_visibility(window);
    install_page(window, deps);
}
