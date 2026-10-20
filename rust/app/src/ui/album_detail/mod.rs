mod edits;
mod info;
mod layout;
mod loading;
mod tracks;
#[cfg(test)]
mod view_tests;
mod viewport;

use slint::{ComponentHandle, ModelRc};

use super::{AlbumDetailState, AppWindow, Context, PlayerState};
use crate::controller::Command;
use edits::{set_rating, toggle_favorite};
use loading::load;
use tracks::{AlbumTracks, loaded_song, loaded_songs};
use viewport::install_viewport;

fn play_all(window: &AppWindow, context: &Context) {
    let songs = loaded_songs(window);
    if !songs.is_empty() {
        context.send(Command::PlaySongs {
            songs,
            song_id: None,
        });
    }
}

fn shuffle_all(window: &AppWindow, context: &Context) {
    let songs = loaded_songs(window);
    if songs.is_empty() {
        return;
    }
    if !window.global::<PlayerState>().get_is_shuffled() {
        context.send(Command::ToggleShuffle);
    }
    context.send(Command::PlaySongs {
        songs,
        song_id: None,
    });
}

fn play_track(window: &AppWindow, context: &Context, song_id: &str) {
    let songs = loaded_songs(window);
    if songs.iter().any(|song| song.id == song_id) {
        context.send(Command::PlaySongs {
            songs,
            song_id: Some(song_id.into()),
        });
    }
}

fn install_load(window: &AppWindow, context: &Context) {
    let weak_window = window.as_weak();
    let context = context.clone();
    window
        .global::<AlbumDetailState>()
        .on_load(move |album_id| {
            if let Some(window) = weak_window.upgrade() {
                load(&window, &context, album_id);
            }
        });
}

fn install_playback(window: &AppWindow, context: &Context) {
    let state = window.global::<AlbumDetailState>();
    let weak_window = window.as_weak();
    let play_context = context.clone();
    state.on_play(move || {
        if let Some(window) = weak_window.upgrade() {
            play_all(&window, &play_context);
        }
    });

    let weak_window = window.as_weak();
    let shuffle_context = context.clone();
    state.on_shuffle(move || {
        if let Some(window) = weak_window.upgrade() {
            shuffle_all(&window, &shuffle_context);
        }
    });

    let weak_window = window.as_weak();
    let track_context = context.clone();
    state.on_play_track(move |song_id| {
        if let Some(window) = weak_window.upgrade() {
            play_track(&window, &track_context, &song_id);
        }
    });
}

fn install_queueing(window: &AppWindow, context: &Context) {
    let state = window.global::<AlbumDetailState>();
    let weak_window = window.as_weak();
    let next_context = context.clone();
    state.on_play_next(move |song_id| {
        if let Some(song) = weak_window
            .upgrade()
            .and_then(|window| loaded_song(&window, &song_id))
        {
            next_context.send(Command::AddNext(vec![song]));
        }
    });

    let weak_window = window.as_weak();
    let queue_context = context.clone();
    state.on_add_to_queue(move |song_id| {
        if let Some(song) = weak_window
            .upgrade()
            .and_then(|window| loaded_song(&window, &song_id))
        {
            queue_context.send(Command::AddLast(vec![song]));
        }
    });
}

fn install_edits(window: &AppWindow, context: &Context) {
    let state = window.global::<AlbumDetailState>();
    let weak_window = window.as_weak();
    let favorite_context = context.clone();
    state.on_toggle_favorite(move || {
        if let Some(window) = weak_window.upgrade() {
            toggle_favorite(&window, &favorite_context);
        }
    });

    let weak_window = window.as_weak();
    let rating_context = context.clone();
    state.on_set_rating(move |rating| {
        if let Some(window) = weak_window.upgrade() {
            set_rating(&window, &rating_context, rating);
        }
    });
}

pub fn install(window: &AppWindow, context: &Context) {
    window
        .global::<AlbumDetailState>()
        .set_tracks(ModelRc::new(AlbumTracks::default()));
    install_viewport(window);
    install_load(window, context);
    install_playback(window, context);
    install_queueing(window, context);
    install_edits(window, context);
}
