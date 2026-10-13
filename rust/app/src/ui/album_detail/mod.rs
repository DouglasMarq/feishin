mod edits;
mod info;
mod loading;
mod tracks;

use slint::{ComponentHandle, ModelRc};

use super::{AlbumDetailState, AppWindow, Context, PlayerState};
use crate::controller::Command;
use edits::{set_rating, toggle_favorite};
use loading::load;
use tracks::{AlbumTracks, loaded_song, loaded_songs};

fn play_all(window: &AppWindow, ctx: &Context) {
    let songs = loaded_songs(window);
    if !songs.is_empty() {
        ctx.send(Command::PlaySongs {
            songs,
            song_id: None,
        });
    }
}

fn shuffle_all(window: &AppWindow, ctx: &Context) {
    let songs = loaded_songs(window);
    if songs.is_empty() {
        return;
    }
    if !window.global::<PlayerState>().get_shuffle() {
        ctx.send(Command::ToggleShuffle);
    }
    ctx.send(Command::PlaySongs {
        songs,
        song_id: None,
    });
}

fn play_track(window: &AppWindow, ctx: &Context, song_id: &str) {
    let songs = loaded_songs(window);
    if songs.iter().any(|song| song.id == song_id) {
        ctx.send(Command::PlaySongs {
            songs,
            song_id: Some(song_id.into()),
        });
    }
}

fn install_load(window: &AppWindow, ctx: &Context) {
    let weak_window = window.as_weak();
    let ctx = ctx.clone();
    window
        .global::<AlbumDetailState>()
        .on_load(move |album_id| {
            if let Some(window) = weak_window.upgrade() {
                load(&window, &ctx, album_id);
            }
        });
}

fn install_playback(window: &AppWindow, ctx: &Context) {
    let state = window.global::<AlbumDetailState>();
    let weak_window = window.as_weak();
    let play_ctx = ctx.clone();
    state.on_play(move || {
        if let Some(window) = weak_window.upgrade() {
            play_all(&window, &play_ctx);
        }
    });

    let weak_window = window.as_weak();
    let shuffle_ctx = ctx.clone();
    state.on_shuffle(move || {
        if let Some(window) = weak_window.upgrade() {
            shuffle_all(&window, &shuffle_ctx);
        }
    });

    let weak_window = window.as_weak();
    let track_ctx = ctx.clone();
    state.on_play_track(move |song_id| {
        if let Some(window) = weak_window.upgrade() {
            play_track(&window, &track_ctx, &song_id);
        }
    });
}

fn install_queueing(window: &AppWindow, ctx: &Context) {
    let state = window.global::<AlbumDetailState>();
    let weak_window = window.as_weak();
    let next_ctx = ctx.clone();
    state.on_play_next(move |song_id| {
        if let Some(song) = weak_window
            .upgrade()
            .and_then(|window| loaded_song(&window, &song_id))
        {
            next_ctx.send(Command::AddNext(vec![song]));
        }
    });

    let weak_window = window.as_weak();
    let queue_ctx = ctx.clone();
    state.on_add_to_queue(move |song_id| {
        if let Some(song) = weak_window
            .upgrade()
            .and_then(|window| loaded_song(&window, &song_id))
        {
            queue_ctx.send(Command::AddLast(vec![song]));
        }
    });
}

fn install_edits(window: &AppWindow, ctx: &Context) {
    let state = window.global::<AlbumDetailState>();
    let weak_window = window.as_weak();
    let favorite_ctx = ctx.clone();
    state.on_toggle_favorite(move || {
        if let Some(window) = weak_window.upgrade() {
            toggle_favorite(&window, &favorite_ctx);
        }
    });

    let weak_window = window.as_weak();
    let rating_ctx = ctx.clone();
    state.on_set_rating(move |rating| {
        if let Some(window) = weak_window.upgrade() {
            set_rating(&window, &rating_ctx, rating);
        }
    });
}

pub fn install(window: &AppWindow, ctx: &Context) {
    window
        .global::<AlbumDetailState>()
        .set_tracks(ModelRc::new(AlbumTracks::default()));
    install_load(window, ctx);
    install_playback(window, ctx);
    install_queueing(window, ctx);
    install_edits(window, ctx);
}
