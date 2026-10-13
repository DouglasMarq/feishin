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

pub fn album_query(sort: i32, desc: bool, search: &str, favorites_only: bool) -> AlbumListQuery {
    let search = search.trim();
    AlbumListQuery {
        sort: usize::try_from(sort)
            .ok()
            .and_then(|i| SORTS.get(i))
            .map_or(AlbumSort::Name, |(sort, _)| *sort),
        order: if desc {
            SortOrder::Desc
        } else {
            SortOrder::Asc
        },
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
        Err(e) => {
            state.set_total(0);
            show_status(window, &format!("Could not load albums: {e:#}"));
        }
    }
}

pub fn install(window: &AppWindow, ctx: &Context) {
    let state = window.global::<AlbumsState>();
    let labels: Vec<SharedString> = SORTS.iter().map(|(_, label)| (*label).into()).collect();
    state.set_sort_options(ModelRc::new(VecModel::from(labels)));

    let weak = window.as_weak();
    let fetch_ctx = ctx.clone();
    let model = AlbumGridModel::new(move |request| {
        spawn_request(weak.clone(), &fetch_ctx, locate, on_count, request)
    });
    state.set_rows(ModelRc::new(model));

    let weak = window.as_weak();
    state.on_load(move |_| {
        if let Some(window) = weak.upgrade() {
            with_model(&window, locate, AlbumGridModel::clear_covers);
            reload(&window);
        }
    });

    let weak = window.as_weak();
    state.on_columns_changed(move |columns| {
        if let Some(window) = weak.upgrade() {
            with_model(&window, locate, |model| {
                model.set_columns(usize::try_from(columns).unwrap_or(1))
            });
        }
    });

    let weak = window.as_weak();
    state.on_query_changed(move || {
        if let Some(window) = weak.upgrade() {
            reload(&window);
        }
    });

    let weak = window.as_weak();
    let debounce = Rc::new(Timer::default());
    state.on_search_edited(move || {
        let weak = weak.clone();
        debounce.start(TimerMode::SingleShot, SEARCH_DEBOUNCE, move || {
            if let Some(window) = weak.upgrade() {
                reload(&window);
            }
        });
    });

    let bind = |action: AlbumAction| {
        let weak = window.as_weak();
        let ctx = ctx.clone();
        move |id: SharedString| {
            if let Some(window) = weak.upgrade() {
                album_action(&window, &ctx, id.into(), action);
            }
        }
    };
    state.on_play(bind(AlbumAction::Play));
    state.on_play_next(bind(AlbumAction::PlayNext));
    state.on_add_to_queue(bind(AlbumAction::AddToQueue));
    state.on_add_to_playlist(bind(AlbumAction::AddToPlaylist));
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
