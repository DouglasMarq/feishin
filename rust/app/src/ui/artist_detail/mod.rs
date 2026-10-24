mod actions;
mod biography;
mod current;
mod info;
mod loading;
mod view;

#[cfg(test)]
mod demo_tests;
#[cfg(test)]
mod view_tests;

use slint::ComponentHandle;

use super::{AppWindow, ArtistDetailState, Context};
use crate::controller::Command;
pub use actions::play_artist;
use actions::{play_album, toggle_favorite};
use current::read_current;
use loading::{load, load_album_cover};

fn install_load(window: &AppWindow, context: &Context) {
    let weak_window = window.as_weak();
    let context = context.clone();
    window
        .global::<ArtistDetailState>()
        .on_load(move |artist_id| {
            if let Some(window) = weak_window.upgrade() {
                load(&window, &context, &artist_id);
            }
        });
}

fn install_play(window: &AppWindow, context: &Context) {
    let weak_window = window.as_weak();
    let context = context.clone();
    window
        .global::<ArtistDetailState>()
        .on_play(move |is_shuffled| {
            let artist_id = read_current(|current| current.id.clone());
            if let (Some(window), false) = (weak_window.upgrade(), artist_id.is_empty()) {
                play_artist(&window, &context, &artist_id, is_shuffled);
            }
        });
}

fn install_toggle_favorite(window: &AppWindow, context: &Context) {
    let weak_window = window.as_weak();
    let context = context.clone();
    window
        .global::<ArtistDetailState>()
        .on_toggle_favorite(move || {
            if let Some(window) = weak_window.upgrade() {
                toggle_favorite(&window, &context);
            }
        });
}

fn install_play_top(window: &AppWindow, context: &Context) {
    let context = context.clone();
    window
        .global::<ArtistDetailState>()
        .on_play_top(move |song_id| {
            let songs = read_current(|current| current.top_songs.clone());
            if !songs.is_empty() {
                context.send(Command::PlaySongs {
                    songs,
                    song_id: Some(song_id.to_string()),
                });
            }
        });
}

fn install_play_album(window: &AppWindow, context: &Context) {
    let weak_window = window.as_weak();
    let context = context.clone();
    window
        .global::<ArtistDetailState>()
        .on_play_album(move |album_id| {
            if let Some(window) = weak_window.upgrade() {
                play_album(&window, &context, album_id.to_string());
            }
        });
}

fn install_album_shown(window: &AppWindow, context: &Context) {
    let weak_window = window.as_weak();
    let context = context.clone();
    window
        .global::<ArtistDetailState>()
        .on_album_shown(move |index| {
            if let (Some(window), Ok(index)) = (weak_window.upgrade(), usize::try_from(index)) {
                load_album_cover(&window, &context, index);
            }
        });
}

pub fn install(window: &AppWindow, context: &Context) {
    install_load(window, context);
    install_play(window, context);
    install_toggle_favorite(window, context);
    install_play_top(window, context);
    install_play_album(window, context);
    install_album_shown(window, context);
}
