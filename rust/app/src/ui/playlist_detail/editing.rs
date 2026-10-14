use std::sync::Arc;

use feishin_core::model::Song;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use super::detail::{RemovalTarget, SharedDetail, lock};
use super::loading::refresh_if_showing;
use crate::ui::{Actions, AppWindow, Context, PlaylistDetailState, on_ui, show_status};

pub fn remove(window: &AppWindow, ctx: &Context, shared: &SharedDetail, indexes: Vec<usize>) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let Some(target) = lock(shared).start_removal(&indexes) else {
        return;
    };
    window.global::<PlaylistDetailState>().set_busy(true);
    let (weak, shared) = (window.as_weak(), Arc::clone(shared));
    ctx.rt.spawn(async move {
        let result = server
            .remove_from_playlist(&target.playlist_id, &indexes)
            .await;
        on_ui(&weak, move |window| {
            finish_removal(window, &shared, &target, indexes.len(), result);
        });
    });
}

fn finish_removal(
    window: &AppWindow,
    shared: &SharedDetail,
    target: &RemovalTarget,
    removed: usize,
    result: feishin_api::Result<()>,
) {
    if !lock(shared).finish_removal(target.generation) {
        return;
    }
    window.global::<PlaylistDetailState>().set_busy(false);
    match result {
        Ok(()) => {
            let message = tr_plural!(
                "Removed {n} song from \"{}\"",
                "Removed {n} songs from \"{}\"",
                removed,
                target.name
            );
            show_status(window, &message);
            refresh_if_showing(window, &target.playlist_id);
        }
        Err(error) => show_status(window, &tr!("Could not remove songs: {}", error)),
    }
}

pub fn add_to_playlist(window: &AppWindow, songs: &[Song]) {
    if songs.is_empty() {
        return;
    }
    let song_ids: Vec<SharedString> = songs.iter().map(|song| song.id.as_str().into()).collect();
    window
        .global::<Actions>()
        .invoke_add_to_playlist(ModelRc::new(VecModel::from(song_ids)));
}
