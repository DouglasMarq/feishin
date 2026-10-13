use std::sync::Arc;

use feishin_core::model::Playlist;
use slint::{ComponentHandle, ModelRc, VecModel};

use super::choices::{filter_playlists, picker_row};
use super::picker::{PickedSongs, SharedPicker, lock};
use crate::ui::playlists::fetch_all;
use crate::ui::{
    AppWindow, Context, PickerRow, PlaylistPickerState, clamp_i32, on_ui, show_status,
};

pub fn show_rows(window: &AppWindow, shared: &SharedPicker) {
    let state = window.global::<PlaylistPickerState>();
    let rows: Vec<PickerRow> = filter_playlists(&lock(shared).playlists, &state.get_filter())
        .into_iter()
        .map(picker_row)
        .collect();
    state.set_rows(ModelRc::new(VecModel::from(rows)));
}

pub fn close(window: &AppWindow, shared: &SharedPicker) {
    lock(shared).restart(PickedSongs::default());
    let state = window.global::<PlaylistPickerState>();
    state.set_open(false);
    state.set_creating(false);
    state.set_busy(false);
    state.set_rows(ModelRc::default());
}

pub fn open(window: &AppWindow, ctx: &Context, shared: &SharedPicker, songs: PickedSongs) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let song_count = songs.song_count();
    if song_count == 0 {
        return;
    }
    let generation = lock(shared).restart(songs);
    show_opened(window, song_count);
    let (weak, shared) = (window.as_weak(), Arc::clone(shared));
    ctx.rt.spawn(async move {
        let result = fetch_all(server.as_ref()).await;
        on_ui(&weak, move |window| {
            apply_playlists(window, &shared, generation, result);
        });
    });
}

fn show_opened(window: &AppWindow, song_count: usize) {
    let state = window.global::<PlaylistPickerState>();
    state.set_song_count(clamp_i32(song_count as u64));
    state.set_filter("".into());
    state.set_rows(ModelRc::default());
    state.set_creating(false);
    state.set_busy(false);
    state.set_loading(true);
    state.set_open(true);
}

fn apply_playlists(
    window: &AppWindow,
    shared: &SharedPicker,
    generation: u64,
    result: feishin_api::Result<Vec<Playlist>>,
) {
    let Some(accepted) = lock(shared).accept_playlists(generation, result) else {
        return;
    };
    if let Err(error) = accepted {
        show_status(window, &format!("Could not load playlists: {error}"));
    }
    window.global::<PlaylistPickerState>().set_loading(false);
    show_rows(window, shared);
}
