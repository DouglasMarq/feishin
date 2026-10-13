use std::sync::Arc;

use feishin_core::model::Song;
use slint::ComponentHandle;

use super::detail::{PlayTarget, SharedDetail, lock};
use super::fetch_all::fetch_all_playlist_songs;
use crate::controller::Command;
use crate::ui::{AppWindow, Context, PlayerState, on_ui, show_status};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayStart {
    FromTop { shuffle: bool },
    AtSong(String),
}

pub fn play_playlist(window: &AppWindow, ctx: &Context, shared: &SharedDetail, start: PlayStart) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let Some(target) = lock(shared).start_play() else {
        return;
    };
    let (weak, play_ctx, shared) = (window.as_weak(), ctx.clone(), Arc::clone(shared));
    ctx.rt.spawn(async move {
        let result = fetch_all_playlist_songs(server.as_ref(), &target.playlist_id).await;
        on_ui(&weak, move |window| {
            finish_play(window, &play_ctx, &shared, &target, start, result);
        });
    });
}

fn finish_play(
    window: &AppWindow,
    ctx: &Context,
    shared: &SharedDetail,
    target: &PlayTarget,
    start: PlayStart,
    result: feishin_api::Result<Vec<Song>>,
) {
    if !lock(shared).is_latest_play(target.request) {
        return;
    }
    match result {
        Ok(songs) if songs.is_empty() => {}
        Ok(songs) => send_play(window, ctx, songs, start),
        Err(error) => show_status(window, &format!("Could not load the playlist: {error}")),
    }
}

fn send_play(window: &AppWindow, ctx: &Context, songs: Vec<Song>, start: PlayStart) {
    let song_id = match start {
        PlayStart::FromTop { shuffle } => {
            if shuffle && !window.global::<PlayerState>().get_shuffle() {
                ctx.send(Command::ToggleShuffle);
            }
            None
        }
        PlayStart::AtSong(song_id) => Some(song_id),
    };
    ctx.send(Command::PlaySongs { songs, song_id });
}
