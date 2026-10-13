use std::rc::Rc;
use std::time::Duration;

use feishin_core::model::{AlbumListQuery, AlbumSort, SortOrder};
use slint::{ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};

use super::album_grid::{AlbumAction, AlbumGridModel, album_action, spawn_request, with_model};
use super::{AlbumGridRow, AlbumsState, AppWindow, Context, clamp_i32, show_status};

const SEARCH_DEBOUNCE: Duration = Duration::from_millis(300);

pub const SORTS: [(AlbumSort, &str); 11] = [
    (AlbumSort::Name, "Name"),
    (AlbumSort::AlbumArtist, "Album artist"),
    (AlbumSort::Year, "Release year"),
    (AlbumSort::RecentlyAdded, "Recently added"),
    (AlbumSort::RecentlyPlayed, "Recently played"),
    (AlbumSort::PlayCount, "Play count"),
    (AlbumSort::Rating, "Rating"),
    (AlbumSort::Favorited, "Favorited"),
    (AlbumSort::SongCount, "Song count"),
    (AlbumSort::Duration, "Duration"),
    (AlbumSort::Random, "Random"),
];

fn sort_at(sort_index: i32) -> AlbumSort {
    usize::try_from(sort_index)
        .ok()
        .and_then(|index| SORTS.get(index))
        .map_or(AlbumSort::Name, |(sort, _)| *sort)
}

fn sort_order(descending: bool) -> SortOrder {
    match descending {
        true => SortOrder::Desc,
        false => SortOrder::Asc,
    }
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
        favorite: favorites_only.then_some(true),
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
            show_status(window, &format!("Could not load albums: {error:#}"));
        }
    }
}

fn install_sort_options(window: &AppWindow) {
    let labels: Vec<SharedString> = SORTS.iter().map(|(_, label)| (*label).into()).collect();
    window
        .global::<AlbumsState>()
        .set_sort_options(ModelRc::new(VecModel::from(labels)));
}

fn install_grid_model(window: &AppWindow, ctx: &Context) {
    let weak_window = window.as_weak();
    let ctx = ctx.clone();
    let model = AlbumGridModel::new(move |request| {
        spawn_request(weak_window.clone(), &ctx, locate, on_count, request)
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

fn install_album_actions(window: &AppWindow, ctx: &Context) {
    let bind = |action: AlbumAction| {
        let weak_window = window.as_weak();
        let ctx = ctx.clone();
        move |album_id: SharedString| {
            if let Some(window) = weak_window.upgrade() {
                album_action(&window, &ctx, album_id.into(), action);
            }
        }
    };
    let state = window.global::<AlbumsState>();
    state.on_play(bind(AlbumAction::Play));
    state.on_play_next(bind(AlbumAction::PlayNext));
    state.on_add_to_queue(bind(AlbumAction::AddToQueue));
    state.on_add_to_playlist(bind(AlbumAction::AddToPlaylist));
}

pub fn install(window: &AppWindow, ctx: &Context) {
    install_sort_options(window);
    install_grid_model(window, ctx);
    install_reload_triggers(window);
    install_columns_changed(window);
    install_album_actions(window, ctx);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queries_follow_the_controls() {
        let query = album_query(2, true, "  river ", true);
        assert_eq!(query.sort, AlbumSort::Year);
        assert_eq!(query.order, SortOrder::Desc);
        assert_eq!(query.search.as_deref(), Some("river"));
        assert_eq!(query.favorite, Some(true));

        let query = album_query(99, false, " ", false);
        assert_eq!(query, AlbumListQuery::default());
        assert_eq!(album_query(-1, false, "", false).sort, AlbumSort::Name);
    }
}
