use slint::ComponentHandle;

use super::actions::{ActionTarget, SongAction, act_on_selection, go_to, set_favorite, set_rating};
use super::model::TracksModel;
use super::row::primary_artist_id;
use super::with_model;
use crate::ui::playlist_picker::PlaylistPicker;
use crate::ui::{AppWindow, Context, Route, TracksState, clamp_i32};

const MAX_RATING: i32 = 5;

fn rating_from(value: i32) -> Option<u8> {
    u8::try_from(value.clamp(0, MAX_RATING)).ok()
}

fn update_selected_count(window: &AppWindow, change: impl FnOnce(&TracksModel) -> usize) {
    if let Some(count) = with_model(window, change) {
        window
            .global::<TracksState>()
            .set_selected_count(clamp_i32(count as u64));
    }
}

fn install_row_selection(window: &AppWindow) {
    let state = window.global::<TracksState>();

    let weak = window.as_weak();
    state.on_select_row(move |index, toggle, extend| {
        let (Some(window), Ok(index)) = (weak.upgrade(), usize::try_from(index)) else {
            return;
        };
        update_selected_count(&window, |model| model.select(index, toggle, extend));
    });

    let weak = window.as_weak();
    state.on_context_row(move |index| {
        let (Some(window), Ok(index)) = (weak.upgrade(), usize::try_from(index)) else {
            return;
        };
        update_selected_count(&window, |model| model.focus(index));
    });

    let weak = window.as_weak();
    state.on_select_all(move || {
        if let Some(window) = weak.upgrade() {
            update_selected_count(&window, TracksModel::select_all);
        }
    });

    let weak = window.as_weak();
    state.on_clear_selection(move || {
        if let Some(window) = weak.upgrade() {
            update_selected_count(&window, TracksModel::clear_selection);
        }
    });
}

fn action_callback(
    window: &AppWindow,
    target: &ActionTarget,
    action: SongAction,
) -> impl Fn() + 'static {
    let weak = window.as_weak();
    let target = target.clone();
    move || {
        if let Some(window) = weak.upgrade() {
            act_on_selection(&window, &target, action);
        }
    }
}

fn install_selection_actions(window: &AppWindow, target: &ActionTarget) {
    let state = window.global::<TracksState>();
    state.on_play_selected(action_callback(window, target, SongAction::Play));
    state.on_add_selected_to_playlist(action_callback(window, target, SongAction::AddToPlaylist));

    let play_next = action_callback(window, target, SongAction::PlayNext);
    let add_last = action_callback(window, target, SongAction::AddLast);
    state.on_queue_selected(move |next| match next {
        true => play_next(),
        false => add_last(),
    });

    let add_favorite = action_callback(window, target, SongAction::Favorite(true));
    let remove_favorite = action_callback(window, target, SongAction::Favorite(false));
    state.on_favorite_selected(move |favorite| match favorite {
        true => add_favorite(),
        false => remove_favorite(),
    });

    let weak = window.as_weak();
    let rate_target = target.clone();
    state.on_rate_selected(move |rating| {
        if let (Some(window), Some(rating)) = (weak.upgrade(), rating_from(rating)) {
            act_on_selection(&window, &rate_target, SongAction::Rate(rating));
        }
    });
}

fn install_navigation(window: &AppWindow, ctx: &Context) {
    let state = window.global::<TracksState>();

    let weak = window.as_weak();
    let album_ctx = ctx.clone();
    state.on_go_to_album(move || {
        if let Some(window) = weak.upgrade() {
            go_to(&window, &album_ctx, Route::AlbumDetail, |song| {
                song.album_id.clone()
            });
        }
    });

    let weak = window.as_weak();
    let artist_ctx = ctx.clone();
    state.on_go_to_artist(move || {
        if let Some(window) = weak.upgrade() {
            go_to(&window, &artist_ctx, Route::ArtistDetail, |song| {
                primary_artist_id(song).map(str::to_owned)
            });
        }
    });
}

fn install_row_edits(window: &AppWindow, ctx: &Context) {
    let state = window.global::<TracksState>();

    let weak = window.as_weak();
    let favorite_ctx = ctx.clone();
    state.on_toggle_favorite(move |index| {
        let (Some(window), Ok(index)) = (weak.upgrade(), usize::try_from(index)) else {
            return;
        };
        if let Some(song) = with_model(&window, |model| model.song_at(index)).flatten() {
            set_favorite(&window, &favorite_ctx, vec![song.id], !song.favorite);
        }
    });

    let weak = window.as_weak();
    let rating_ctx = ctx.clone();
    state.on_rate_row(move |index, rating| {
        let (Some(window), Ok(index), Some(rating)) =
            (weak.upgrade(), usize::try_from(index), rating_from(rating))
        else {
            return;
        };
        if let Some(id) = with_model(&window, |model| model.song_id_at(index)).flatten() {
            set_rating(&window, &rating_ctx, vec![id], rating);
        }
    });
}

pub(super) fn install(window: &AppWindow, ctx: &Context, picker: &PlaylistPicker) {
    let target = ActionTarget {
        ctx: ctx.clone(),
        picker: picker.clone(),
    };
    install_row_selection(window);
    install_selection_actions(window, &target);
    install_navigation(window, ctx);
    install_row_edits(window, ctx);
}
