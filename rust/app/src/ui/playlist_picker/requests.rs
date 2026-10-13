use std::sync::Arc;

use feishin_api::MusicServer;
use feishin_core::model::Playlist;
use slint::ComponentHandle;

use super::choices::new_song_ids;
use super::dialog::close;
use super::picker::{PendingRequest, SharedPicker, lock};
use crate::ui::playlists::song_count_label;
use crate::ui::{
    AppWindow, Context, PlaylistPickerState, on_ui, playlist_detail, playlists, show_status,
};

enum RequestOutcome {
    Succeeded(String),
    Failed(String),
}

struct PickedPlaylist {
    id: String,
    name: String,
}

fn start_request(window: &AppWindow, shared: &SharedPicker) -> Option<PendingRequest> {
    let pending = lock(shared).start_request()?;
    window.global::<PlaylistPickerState>().set_busy(true);
    Some(pending)
}

fn finish_request(
    window: &AppWindow,
    shared: &SharedPicker,
    generation: u64,
    outcome: RequestOutcome,
) {
    if !lock(shared).finish_request(generation) {
        return;
    }
    window.global::<PlaylistPickerState>().set_busy(false);
    match outcome {
        RequestOutcome::Succeeded(message) => {
            close(window, shared);
            show_status(window, &message);
        }
        RequestOutcome::Failed(message) => show_status(window, &message),
    }
}

pub fn pick(window: &AppWindow, ctx: &Context, shared: &SharedPicker, playlist_id: String) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let Some(PendingRequest {
        generation,
        song_ids,
    }) = start_request(window, shared)
    else {
        return;
    };
    let picked = PickedPlaylist {
        name: lock(shared).playlist_name(&playlist_id),
        id: playlist_id,
    };
    let (weak, shared) = (window.as_weak(), Arc::clone(shared));
    ctx.rt.spawn(async move {
        let outcome = add_new_songs(server.as_ref(), &picked.id, &song_ids).await;
        on_ui(&weak, move |window| {
            finish_pick(window, &shared, generation, &picked, outcome);
        });
    });
}

async fn add_new_songs(
    server: &dyn MusicServer,
    playlist_id: &str,
    song_ids: &[String],
) -> feishin_api::Result<usize> {
    let existing = server.playlist_songs(playlist_id).await?;
    let added_ids = new_song_ids(&existing, song_ids);
    if !added_ids.is_empty() {
        server.add_to_playlist(playlist_id, &added_ids).await?;
    }
    Ok(added_ids.len())
}

fn finish_pick(
    window: &AppWindow,
    shared: &SharedPicker,
    generation: u64,
    picked: &PickedPlaylist,
    outcome: feishin_api::Result<usize>,
) {
    let name = &picked.name;
    let changed = matches!(outcome, Ok(added) if added > 0);
    let outcome = match outcome {
        Ok(0) => RequestOutcome::Succeeded(format!("Already in \"{name}\"")),
        Ok(added) => {
            RequestOutcome::Succeeded(format!("Added {} to \"{name}\"", song_count_label(added)))
        }
        Err(error) => RequestOutcome::Failed(format!("Could not add to \"{name}\": {error}")),
    };
    finish_request(window, shared, generation, outcome);
    if changed {
        playlists::refresh(window);
        playlist_detail::refresh_if_showing(window, &picked.id);
    }
}

pub fn create(window: &AppWindow, ctx: &Context, shared: &SharedPicker, name: &str) {
    let name = name.trim().to_owned();
    let Some(server) = ctx.session.get().filter(|_| !name.is_empty()) else {
        return;
    };
    let Some(PendingRequest {
        generation,
        song_ids,
    }) = start_request(window, shared)
    else {
        return;
    };
    let song_ids = new_song_ids(&[], &song_ids);
    let (weak, shared) = (window.as_weak(), Arc::clone(shared));
    ctx.rt.spawn(async move {
        let result = server.create_playlist(&name, &song_ids).await;
        on_ui(&weak, move |window| {
            finish_create(window, &shared, generation, song_ids.len(), result);
        });
    });
}

fn finish_create(
    window: &AppWindow,
    shared: &SharedPicker,
    generation: u64,
    song_count: usize,
    result: feishin_api::Result<Playlist>,
) {
    let created = result.is_ok();
    let outcome = match result {
        Ok(playlist) => RequestOutcome::Succeeded(format!(
            "Created \"{}\" with {}",
            playlist.name,
            song_count_label(song_count)
        )),
        Err(error) => RequestOutcome::Failed(format!("Could not create the playlist: {error}")),
    };
    finish_request(window, shared, generation, outcome);
    if created {
        playlists::refresh(window);
    }
}
