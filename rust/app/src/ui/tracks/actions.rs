use std::sync::Arc;

use feishin_api::{ApiError, MusicServer};
use feishin_core::model::{ItemKind, Song};
use slint::{ComponentHandle, SharedString};

use super::listed_actions::act_on_listed;
use super::model::TracksModel;
use super::selection::{FirstSelected, SelectedSongs};
use super::{string_model, with_model};
use crate::controller::Command;
use crate::ui::playlist_picker::PlaylistPicker;
use crate::ui::{Actions, AppState, AppWindow, Context, Route, on_ui, show_status};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SongAction {
    Play,
    PlayNext,
    AddLast,
    AddToPlaylist,
    Favorite(bool),
    Rate(u8),
}

#[derive(Clone)]
pub(super) struct ActionTarget {
    pub context: Context,
    pub picker: PlaylistPicker,
}

pub(super) fn act_on_selection(window: &AppWindow, target: &ActionTarget, action: SongAction) {
    match with_model(window, TracksModel::selected_songs) {
        Some(SelectedSongs::Loaded(songs)) => perform(window, &target.context, action, songs),
        Some(SelectedSongs::Listed(listed)) => act_on_listed(window, target, action, listed),
        None => {}
    }
}

fn perform(window: &AppWindow, context: &Context, action: SongAction, songs: Vec<Song>) {
    if songs.is_empty() {
        return;
    }
    match action {
        SongAction::Play => context.send(Command::PlaySongs {
            songs,
            song_id: None,
        }),
        SongAction::PlayNext => context.send(Command::AddNext(songs)),
        SongAction::AddLast => context.send(Command::AddLast(songs)),
        SongAction::AddToPlaylist => open_playlist_picker(window, &songs),
        SongAction::Favorite(favorite) => set_favorite(window, context, ids_of(&songs), favorite),
        SongAction::Rate(rating) => set_rating(window, context, ids_of(&songs), rating),
    }
}

fn open_playlist_picker(window: &AppWindow, songs: &[Song]) {
    let ids: Vec<SharedString> = songs.iter().map(|song| song.id.as_str().into()).collect();
    window
        .global::<Actions>()
        .invoke_add_to_playlist(string_model(ids));
}

fn ids_of(songs: &[Song]) -> Vec<String> {
    songs.iter().map(|song| song.id.clone()).collect()
}

pub(super) fn set_favorite(
    window: &AppWindow,
    context: &Context,
    ids: Vec<String>,
    favorite: bool,
) {
    let Some(server) = context.session.get() else {
        return;
    };
    let weak = window.as_weak();
    context.runtime.spawn(async move {
        let result = server.set_favorite(ItemKind::Song, &ids, favorite).await;
        on_ui(&weak, move |window| match result {
            Ok(()) => {
                with_model(window, |model| {
                    model.update_songs(&ids, |song| song.is_favorite = favorite);
                });
            }
            Err(error) => show_status(window, &tr!("Could not update favorites: {}", error)),
        });
    });
}

pub(super) async fn rate_until_failure(
    server: Arc<dyn MusicServer>,
    ids: Vec<String>,
    rating: u8,
) -> (Vec<String>, Option<ApiError>) {
    let mut rated = Vec::new();
    for id in ids {
        if let Err(error) = server.set_rating(ItemKind::Song, &id, rating).await {
            return (rated, Some(error));
        }
        rated.push(id);
    }
    (rated, None)
}

pub(super) fn set_rating(window: &AppWindow, context: &Context, ids: Vec<String>, rating: u8) {
    let Some(server) = context.session.get() else {
        return;
    };
    let weak = window.as_weak();
    let work = context.session.background_work();
    context.runtime.spawn(async move {
        let Some((rated, failure)) = work.run(rate_until_failure(server, ids, rating)).await else {
            return;
        };
        on_ui(&weak, move |window| {
            let user_rating = (rating > 0).then_some(rating);
            with_model(window, |model| {
                model.update_songs(&rated, |song| song.user_rating = user_rating);
            });
            if let Some(error) = failure {
                show_status(window, &tr!("Could not set the rating: {}", error));
            }
        });
    });
}

pub(super) fn go_to(
    window: &AppWindow,
    context: &Context,
    route: Route,
    target: impl FnOnce(&Song) -> Option<String> + Send + 'static,
) {
    let Some(first) = with_model(window, TracksModel::first_selected).flatten() else {
        return;
    };
    with_first_song(window, context, first, move |window, song| {
        if let Some(target_id) = target(&song) {
            window
                .global::<AppState>()
                .invoke_navigate(route, target_id.into());
        }
    });
}

fn with_first_song(
    window: &AppWindow,
    context: &Context,
    first: FirstSelected,
    use_song: impl FnOnce(&AppWindow, Song) + Send + 'static,
) {
    if let Some(song) = first.song {
        use_song(window, song);
        return;
    }
    let Some(server) = context.session.get() else {
        return;
    };
    let weak = window.as_weak();
    context.runtime.spawn(async move {
        let mut chunks = first.listed.into_chunks(1);
        let result = chunks.next_songs(server.as_ref()).await;
        on_ui(&weak, move |window| match result {
            Ok(songs) => {
                if let Some(song) = songs.into_iter().flatten().next() {
                    use_song(window, song);
                }
            }
            Err(error) => show_status(window, &tr!("Could not load the selection: {}", error)),
        });
    });
}
