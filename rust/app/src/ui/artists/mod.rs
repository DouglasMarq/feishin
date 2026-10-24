mod item;
mod list;
mod models;
mod query;
mod requests;

#[cfg(test)]
mod view_tests;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};

use super::artist_detail::play_artist;
use super::{AppWindow, ArtistsState, Context};
pub use item::song_count_label;
use list::ArtistList;
use models::{GridModel, ListModel};
use query::{SORTS, sort_label};
use requests::{reload, spawn_request, with_list};

const SEARCH_DEBOUNCE: Duration = Duration::from_millis(300);

type SearchText = Rc<RefCell<String>>;

pub fn set_favorite(window: &AppWindow, artist_id: &str, is_favorite: bool) {
    with_list(window, |list| list.set_favorite(artist_id, is_favorite));
}

fn install_models(window: &AppWindow, context: &Context) {
    let weak_window = window.as_weak();
    let context = context.clone();
    let list = Rc::new(ArtistList::new(move |request| {
        spawn_request(weak_window.clone(), &context, request);
    }));
    let state = window.global::<ArtistsState>();
    state.set_items(ModelRc::new(ListModel {
        list: Rc::clone(&list),
    }));
    state.set_grid_rows(ModelRc::new(GridModel { list }));
    show_sort_options(window);
}

pub fn show_sort_options(window: &AppWindow) {
    let options: Vec<SharedString> = SORTS.iter().map(|sort| sort_label(*sort).into()).collect();
    window
        .global::<ArtistsState>()
        .set_sort_options(ModelRc::new(VecModel::from(options)));
}

fn install_load(window: &AppWindow, context: &Context, search: SearchText) {
    let weak_window = window.as_weak();
    let session = context.session.clone();
    window.global::<ArtistsState>().on_load(move |_| {
        if let (Some(window), true) = (weak_window.upgrade(), session.get().is_some()) {
            reload(&window, &search.borrow());
        }
    });
}

fn install_query_changed(window: &AppWindow, search: SearchText) {
    let weak_window = window.as_weak();
    window.global::<ArtistsState>().on_query_changed(move || {
        if let Some(window) = weak_window.upgrade() {
            reload(&window, &search.borrow());
        }
    });
}

fn install_search_edited(window: &AppWindow, search: SearchText) {
    let weak_window = window.as_weak();
    let debounce = Rc::new(Timer::default());
    window
        .global::<ArtistsState>()
        .on_search_edited(move |text| {
            let weak_window = weak_window.clone();
            let search = Rc::clone(&search);
            debounce.start(TimerMode::SingleShot, SEARCH_DEBOUNCE, move || {
                let Some(window) = weak_window.upgrade() else {
                    return;
                };
                *search.borrow_mut() = text.trim().to_owned();
                reload(&window, &search.borrow());
            });
        });
}

fn install_set_columns(window: &AppWindow) {
    let weak_window = window.as_weak();
    window
        .global::<ArtistsState>()
        .on_set_columns(move |columns| {
            if let Some(window) = weak_window.upgrade() {
                with_list(&window, |list| {
                    list.set_columns(usize::try_from(columns).unwrap_or(1));
                });
            }
        });
}

fn install_play(window: &AppWindow, context: &Context) {
    let weak_window = window.as_weak();
    let context = context.clone();
    window.global::<ArtistsState>().on_play(move |artist_id| {
        if let Some(window) = weak_window.upgrade() {
            play_artist(&window, &context, artist_id.as_str(), false);
        }
    });
}

pub fn install(window: &AppWindow, context: &Context) {
    install_models(window, context);
    let search = SearchText::default();
    install_load(window, context, Rc::clone(&search));
    install_query_changed(window, Rc::clone(&search));
    install_search_edited(window, search);
    install_set_columns(window);
    install_play(window, context);
}
