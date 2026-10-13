mod cover_lru;
mod item;
mod list;
mod models;
mod query;
mod requests;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};

use super::artist_detail::play_artist;
use super::{AppWindow, ArtistsState, Context};
pub use cover_lru::CoverLru;
pub use item::count_label;
use list::ArtistList;
use models::{GridModel, ListModel};
use query::SORTS;
use requests::{reload, spawn_request, with_list};

const SEARCH_DEBOUNCE: Duration = Duration::from_millis(300);

type SearchText = Rc<RefCell<String>>;

pub fn set_favorite(window: &AppWindow, artist_id: &str, favorite: bool) {
    with_list(window, |list| list.set_favorite(artist_id, favorite));
}

fn install_models(window: &AppWindow, ctx: &Context) -> Rc<ArtistList> {
    let weak_window = window.as_weak();
    let ctx = ctx.clone();
    let list = Rc::new(ArtistList::new(move |request| {
        spawn_request(weak_window.clone(), &ctx, request);
    }));
    let state = window.global::<ArtistsState>();
    state.set_items(ModelRc::new(ListModel {
        list: Rc::clone(&list),
    }));
    state.set_grid_rows(ModelRc::new(GridModel {
        list: Rc::clone(&list),
    }));
    let options: Vec<SharedString> = SORTS.iter().map(|(label, _)| (*label).into()).collect();
    state.set_sort_options(ModelRc::new(VecModel::from(options)));
    list
}

fn install_load(window: &AppWindow, ctx: &Context, list: Rc<ArtistList>, search: SearchText) {
    let weak_window = window.as_weak();
    let session = ctx.session.clone();
    let loaded_for_server: Rc<RefCell<Option<String>>> = Rc::default();
    window.global::<ArtistsState>().on_load(move |_| {
        let (Some(window), Some(server)) = (weak_window.upgrade(), session.get()) else {
            return;
        };
        let server_id = server.config().id.clone();
        let mut loaded_for_server = loaded_for_server.borrow_mut();
        if loaded_for_server.as_deref() != Some(server_id.as_str()) || list.generation() == 0 {
            *loaded_for_server = Some(server_id);
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

fn install_play(window: &AppWindow, ctx: &Context) {
    let weak_window = window.as_weak();
    let ctx = ctx.clone();
    window.global::<ArtistsState>().on_play(move |artist_id| {
        if let Some(window) = weak_window.upgrade() {
            play_artist(&window, &ctx, artist_id.as_str(), false);
        }
    });
}

pub fn install(window: &AppWindow, ctx: &Context) {
    let list = install_models(window, ctx);
    let search = SearchText::default();
    install_load(window, ctx, list, Rc::clone(&search));
    install_query_changed(window, Rc::clone(&search));
    install_search_edited(window, search);
    install_set_columns(window);
    install_play(window, ctx);
}
