use std::sync::Arc;

use feishin_core::model::Song;
use slint::ComponentHandle;

use super::detail::{SharedDetail, lock};
use super::editing::{add_to_playlist, remove};
use super::loading::load;
use super::selecting::install_selection;
use crate::controller::Command;
use crate::ui::playlists::PlayMode;
use crate::ui::{AppWindow, Context, PlayerState, PlaylistDetailState, PlaylistSongAction};

pub fn install(window: &AppWindow, ctx: &Context) {
    let shared: SharedDetail = Arc::default();
    install_load(window, ctx, &shared);
    install_play(window, ctx, &shared);
    install_play_row(window, ctx, &shared);
    install_selection(window, &shared);
    install_selection_actions(window, ctx, &shared);
    install_context_action(window, ctx, &shared);
}

fn install_load(window: &AppWindow, ctx: &Context, shared: &SharedDetail) {
    let (weak, load_ctx, load_shared) = (window.as_weak(), ctx.clone(), Arc::clone(shared));
    window
        .global::<PlaylistDetailState>()
        .on_load(move |playlist_id| {
            if let Some(window) = weak.upgrade() {
                load(&window, &load_ctx, &load_shared, &playlist_id);
            }
        });
}

fn install_play(window: &AppWindow, ctx: &Context, shared: &SharedDetail) {
    let (weak, play_ctx, play_shared) = (window.as_weak(), ctx.clone(), Arc::clone(shared));
    window
        .global::<PlaylistDetailState>()
        .on_play(move |shuffle| {
            let Some(window) = weak.upgrade() else {
                return;
            };
            let songs = lock(&play_shared).songs.clone();
            if songs.is_empty() {
                return;
            }
            if shuffle && !window.global::<PlayerState>().get_shuffle() {
                play_ctx.send(Command::ToggleShuffle);
            }
            play_ctx.send(Command::PlaySongs {
                songs,
                song_id: None,
            });
        });
}

fn install_play_row(window: &AppWindow, ctx: &Context, shared: &SharedDetail) {
    let (row_ctx, row_shared) = (ctx.clone(), Arc::clone(shared));
    window
        .global::<PlaylistDetailState>()
        .on_play_row(move |index| {
            let detail = lock(&row_shared);
            let Some(song) = usize::try_from(index)
                .ok()
                .and_then(|index| detail.songs.get(index))
            else {
                return;
            };
            row_ctx.send(Command::PlaySongs {
                song_id: Some(song.id.clone()),
                songs: detail.songs.clone(),
            });
        });
}

fn install_selection_actions(window: &AppWindow, ctx: &Context, shared: &SharedDetail) {
    let state = window.global::<PlaylistDetailState>();

    let (weak, remove_ctx, remove_shared) = (window.as_weak(), ctx.clone(), Arc::clone(shared));
    state.on_remove_selected(move || {
        if let Some(window) = weak.upgrade() {
            let indexes = lock(&remove_shared).selection.indexes();
            remove(&window, &remove_ctx, &remove_shared, indexes);
        }
    });

    let (weak, add_shared) = (window.as_weak(), Arc::clone(shared));
    state.on_add_selected_to_playlist(move || {
        if let Some(window) = weak.upgrade() {
            let songs = {
                let detail = lock(&add_shared);
                detail.songs_at(&detail.selection.indexes())
            };
            add_to_playlist(&window, &songs);
        }
    });
}

fn install_context_action(window: &AppWindow, ctx: &Context, shared: &SharedDetail) {
    let (weak, menu_ctx, menu_shared) = (window.as_weak(), ctx.clone(), Arc::clone(shared));
    window
        .global::<PlaylistDetailState>()
        .on_context_action(move |index, action| {
            let (Some(window), Ok(index)) = (weak.upgrade(), usize::try_from(index)) else {
                return;
            };
            let (targets, songs) = {
                let detail = lock(&menu_shared);
                let targets = detail.selection.targets(index);
                let songs = detail.songs_at(&targets);
                (targets, songs)
            };
            if songs.is_empty() {
                return;
            }
            let menu_target = MenuTarget { targets, songs };
            run_context_action(&window, &menu_ctx, &menu_shared, action, menu_target);
        });
}

struct MenuTarget {
    targets: Vec<usize>,
    songs: Vec<Song>,
}

fn run_context_action(
    window: &AppWindow,
    ctx: &Context,
    shared: &SharedDetail,
    action: PlaylistSongAction,
    menu_target: MenuTarget,
) {
    let MenuTarget { targets, songs } = menu_target;
    match action {
        PlaylistSongAction::AddToPlaylist => add_to_playlist(window, &songs),
        PlaylistSongAction::Remove => remove(window, ctx, shared, targets),
        PlaylistSongAction::Play => ctx.send(PlayMode::Now.command(songs)),
        PlaylistSongAction::PlayNext => ctx.send(PlayMode::Next.command(songs)),
        PlaylistSongAction::AddToQueue => ctx.send(PlayMode::Last.command(songs)),
    }
}
