use std::rc::Rc;
use std::time::Duration;

use feishin_core::model::{AlbumListQuery, AlbumSort};
use slint::{ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};

use super::album_grid::{AlbumAction, AlbumGridModel, album_action, spawn_request, with_model};
use super::{AlbumGridRow, AlbumsState, AppWindow, Context, clamp_i32, show_status, sort_order};

const SEARCH_DEBOUNCE: Duration = Duration::from_millis(300);

pub const SORTS: [AlbumSort; 11] = [
    AlbumSort::Name,
    AlbumSort::AlbumArtist,
    AlbumSort::Year,
    AlbumSort::RecentlyAdded,
    AlbumSort::RecentlyPlayed,
    AlbumSort::PlayCount,
    AlbumSort::Rating,
    AlbumSort::Favorited,
    AlbumSort::SongCount,
    AlbumSort::Duration,
    AlbumSort::Random,
];

fn sort_label(sort: AlbumSort) -> String {
    match sort {
        AlbumSort::Name => tr!("Name"),
        AlbumSort::AlbumArtist => tr!("Album artist"),
        AlbumSort::Year => tr!("Release year"),
        AlbumSort::RecentlyAdded => tr!("Recently added"),
        AlbumSort::RecentlyPlayed => tr!("Recently played"),
        AlbumSort::PlayCount => tr!("Play count"),
        AlbumSort::Rating => tr!("Rating"),
        AlbumSort::Favorited => tr!("Favorited"),
        AlbumSort::SongCount => tr!("Song count"),
        AlbumSort::Duration => tr!("Duration"),
        AlbumSort::Random => tr!("Random"),
    }
}

fn sort_at(sort_index: i32) -> AlbumSort {
    usize::try_from(sort_index)
        .ok()
        .and_then(|index| SORTS.get(index))
        .copied()
        .unwrap_or(AlbumSort::Name)
}

pub fn album_query(
    sort_index: i32,
    descending: bool,
    search: &str,
    favorites_only: bool,
) -> AlbumListQuery {
    let search = search.trim();
    AlbumListQuery {
        sort: sort_at(sort_index),
        order: sort_order(descending),
        search: (!search.is_empty()).then(|| search.to_owned()),
        is_favorite: favorites_only.then_some(true),
        ..AlbumListQuery::default()
    }
}

fn locate(window: &AppWindow) -> ModelRc<AlbumGridRow> {
    window.global::<AlbumsState>().get_rows()
}

fn current_query(window: &AppWindow) -> AlbumListQuery {
    let state = window.global::<AlbumsState>();
    album_query(
        state.get_sort(),
        state.get_sort_desc(),
        &state.get_search(),
        state.get_favorites_only(),
    )
}

fn reload(window: &AppWindow) {
    let query = current_query(window);
    window.global::<AlbumsState>().set_loading(true);
    with_model(window, locate, |model| model.reset(query));
}

fn on_count(window: &AppWindow, result: anyhow::Result<usize>) {
    let state = window.global::<AlbumsState>();
    state.set_loading(false);
    match result {
        Ok(total) => state.set_total(clamp_i32(total as u64)),
        Err(error) => {
            state.set_total(0);
            show_status(
                window,
                &tr!("Could not load albums: {}", format!("{error:#}")),
            );
        }
    }
}

pub fn show_sort_options(window: &AppWindow) {
    let labels: Vec<SharedString> = SORTS.iter().map(|sort| sort_label(*sort).into()).collect();
    window
        .global::<AlbumsState>()
        .set_sort_options(ModelRc::new(VecModel::from(labels)));
}

fn install_grid_model(window: &AppWindow, context: &Context) {
    let weak_window = window.as_weak();
    let context = context.clone();
    let model = AlbumGridModel::new(move |request| {
        spawn_request(weak_window.clone(), &context, locate, on_count, request)
    });
    window.global::<AlbumsState>().set_rows(ModelRc::new(model));
}

fn install_reload_triggers(window: &AppWindow) {
    let state = window.global::<AlbumsState>();
    let weak_window = window.as_weak();
    state.on_load(move |_| {
        if let Some(window) = weak_window.upgrade() {
            with_model(&window, locate, AlbumGridModel::clear_covers);
            reload(&window);
        }
    });

    let weak_window = window.as_weak();
    state.on_query_changed(move || {
        if let Some(window) = weak_window.upgrade() {
            reload(&window);
        }
    });

    let weak_window = window.as_weak();
    let debounce = Rc::new(Timer::default());
    state.on_search_edited(move || {
        let weak_window = weak_window.clone();
        debounce.start(TimerMode::SingleShot, SEARCH_DEBOUNCE, move || {
            if let Some(window) = weak_window.upgrade() {
                reload(&window);
            }
        });
    });
}

fn install_columns_changed(window: &AppWindow) {
    let weak_window = window.as_weak();
    window
        .global::<AlbumsState>()
        .on_columns_changed(move |columns| {
            if let Some(window) = weak_window.upgrade() {
                with_model(&window, locate, |model| {
                    model.set_columns(usize::try_from(columns).unwrap_or(1))
                });
            }
        });
}

fn install_album_actions(window: &AppWindow, context: &Context) {
    let bind = |action: AlbumAction| {
        let weak_window = window.as_weak();
        let context = context.clone();
        move |album_id: SharedString| {
            if let Some(window) = weak_window.upgrade() {
                album_action(&window, &context, album_id.into(), action);
            }
        }
    };
    let state = window.global::<AlbumsState>();
    state.on_play(bind(AlbumAction::Play));
    state.on_play_next(bind(AlbumAction::PlayNext));
    state.on_add_to_queue(bind(AlbumAction::AddToQueue));
    state.on_add_to_playlist(bind(AlbumAction::AddToPlaylist));
}

pub fn install(window: &AppWindow, context: &Context) {
    show_sort_options(window);
    install_grid_model(window, context);
    install_reload_triggers(window);
    install_columns_changed(window);
    install_album_actions(window, context);
}

#[cfg(test)]
mod tests {
    use feishin_core::model::SortOrder;

    use super::*;

    #[test]
    fn queries_follow_the_controls() {
        let query = album_query(2, true, "  river ", true);
        assert_eq!(query.sort, AlbumSort::Year);
        assert_eq!(query.order, SortOrder::Desc);
        assert_eq!(query.search.as_deref(), Some("river"));
        assert_eq!(query.is_favorite, Some(true));

        let query = album_query(99, false, " ", false);
        assert_eq!(query, AlbumListQuery::default());
        assert_eq!(album_query(-1, false, "", false).sort, AlbumSort::Name);
    }
}
