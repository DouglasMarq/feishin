mod actions;
mod biography;
mod current;
mod info;
mod loading;
mod view;

#[cfg(test)]
mod demo_tests;

use slint::ComponentHandle;

use super::{AppWindow, ArtistDetailState, Context};
use crate::controller::Command;
pub use actions::play_artist;
use actions::{play_album, toggle_favorite};
use current::read_current;
use loading::{load, load_album_cover};

fn install_load(window: &AppWindow, ctx: &Context) {
    let weak_window = window.as_weak();
    let ctx = ctx.clone();
    window
        .global::<ArtistDetailState>()
        .on_load(move |artist_id| {
            if let Some(window) = weak_window.upgrade() {
                load(&window, &ctx, &artist_id);
            }
        });
}

fn install_play(window: &AppWindow, ctx: &Context) {
    let weak_window = window.as_weak();
    let ctx = ctx.clone();
    window
        .global::<ArtistDetailState>()
        .on_play(move |shuffle| {
            let artist_id = read_current(|current| current.id.clone());
            if let (Some(window), false) = (weak_window.upgrade(), artist_id.is_empty()) {
                play_artist(&window, &ctx, &artist_id, shuffle);
            }
        });
}

fn install_toggle_favorite(window: &AppWindow, ctx: &Context) {
    let weak_window = window.as_weak();
    let ctx = ctx.clone();
    window
        .global::<ArtistDetailState>()
        .on_toggle_favorite(move || {
            if let Some(window) = weak_window.upgrade() {
                toggle_favorite(&window, &ctx);
            }
        });
}

fn install_play_top(window: &AppWindow, ctx: &Context) {
    let ctx = ctx.clone();
    window
        .global::<ArtistDetailState>()
        .on_play_top(move |song_id| {
            let songs = read_current(|current| current.top_songs.clone());
            if !songs.is_empty() {
                ctx.send(Command::PlaySongs {
                    songs,
                    song_id: Some(song_id.to_string()),
                });
            }
        });
}

fn install_play_album(window: &AppWindow, ctx: &Context) {
    let weak_window = window.as_weak();
    let ctx = ctx.clone();
    window
        .global::<ArtistDetailState>()
        .on_play_album(move |album_id| {
            if let Some(window) = weak_window.upgrade() {
                play_album(&window, &ctx, album_id.to_string());
            }
        });
}

fn install_album_shown(window: &AppWindow, ctx: &Context) {
    let weak_window = window.as_weak();
    let ctx = ctx.clone();
    window
        .global::<ArtistDetailState>()
        .on_album_shown(move |index| {
            if let (Some(window), Ok(index)) = (weak_window.upgrade(), usize::try_from(index)) {
                load_album_cover(&window, &ctx, index);
            }
        });
}

pub fn install(window: &AppWindow, ctx: &Context) {
    install_load(window, ctx);
    install_play(window, ctx);
    install_toggle_favorite(window, ctx);
    install_play_top(window, ctx);
    install_play_album(window, ctx);
    install_album_shown(window, ctx);
}
