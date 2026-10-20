use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, SharedString};

use super::actions::play_track;
use super::binding::{Binding, set_grid_columns, with_grid, with_table};
use super::card::CardSource;
use super::genre_list::{
    GenreList, fetch_genres, genre_album_query, genre_song_query, show_filtered_genres, show_genre,
};
use super::grid_loader::card_grid;
use super::table_loader::track_table;
use crate::ui::{AppWindow, CardRow, Context, GenresState, TrackItem};

type SharedGenreList = Rc<RefCell<GenreList>>;

const ALBUMS: Binding<CardRow> = Binding {
    rows: |window| window.global::<GenresState>().get_albums(),
    loading: |window, is_loading| {
        window
            .global::<GenresState>()
            .set_is_albums_loading(is_loading)
    },
};

const TRACKS: Binding<TrackItem> = Binding {
    rows: |window| window.global::<GenresState>().get_tracks(),
    loading: |window, is_loading| {
        window
            .global::<GenresState>()
            .set_is_tracks_loading(is_loading)
    },
};

pub fn install(window: &AppWindow, context: &Context) {
    let state = window.global::<GenresState>();
    state.set_albums(card_grid(window, context, ALBUMS));
    state.set_tracks(track_table(window, context, false, TRACKS));
    let list: SharedGenreList = Rc::default();
    install_load(window, context, &list);
    install_filter(window, list);
    install_album_columns(window);
    install_play_track(window, context);
}

fn install_load(window: &AppWindow, context: &Context, list: &SharedGenreList) {
    let weak = window.as_weak();
    let load_context = context.clone();
    let load_list = Rc::clone(list);
    window.global::<GenresState>().on_load(move |genre_id| {
        let Some(window) = weak.upgrade() else {
            return;
        };
        if genre_id.is_empty() {
            show_genre_index(&window, &load_context, &load_list);
            return;
        }
        open_genre(&window, &load_context, &load_list, &genre_id);
    });
}

fn show_genre_index(window: &AppWindow, context: &Context, list: &SharedGenreList) {
    with_grid(window, ALBUMS, |grid| grid.reset(None));
    list.borrow_mut().filter.clear();
    fetch_genres(window, context, list);
}

fn open_genre(
    window: &AppWindow,
    context: &Context,
    list: &SharedGenreList,
    genre_id: &SharedString,
) {
    let state = window.global::<GenresState>();
    state.set_is_albums_loading(true);
    state.set_is_tracks_loading(true);
    with_grid(window, ALBUMS, |grid| {
        grid.reset(Some(CardSource::Albums(genre_album_query(genre_id))));
    });
    with_table(window, TRACKS, |table| {
        table.reset(genre_song_query(genre_id))
    });
    show_genre(window, &list.borrow());
    let is_known = list.borrow().contains(genre_id.as_str());
    if !is_known {
        fetch_genres(window, context, list);
    }
}

fn install_filter(window: &AppWindow, list: SharedGenreList) {
    let weak = window.as_weak();
    window
        .global::<GenresState>()
        .on_filter_edited(move |text| {
            let Some(window) = weak.upgrade() else {
                return;
            };
            let mut list = list.borrow_mut();
            list.filter = text.to_string();
            show_filtered_genres(&window, &list);
        });
}

fn install_album_columns(window: &AppWindow) {
    let weak = window.as_weak();
    window
        .global::<GenresState>()
        .on_album_columns(move |columns| {
            if let Some(window) = weak.upgrade() {
                set_grid_columns(&window, ALBUMS, columns);
            }
        });
}

fn install_play_track(window: &AppWindow, context: &Context) {
    let weak = window.as_weak();
    let play_context = context.clone();
    window.global::<GenresState>().on_play_track(move |index| {
        if let Some(window) = weak.upgrade() {
            play_track(&window, &play_context, TRACKS, index);
        }
    });
}
