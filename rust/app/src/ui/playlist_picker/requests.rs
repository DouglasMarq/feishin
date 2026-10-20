use std::sync::Arc;

use feishin_api::MusicServer;
use feishin_core::model::Playlist;
use slint::ComponentHandle;

use super::choices::new_song_ids;
use super::dialog::close;
use super::listed_requests::{PickedPlaylist, add_listed, create_listed};
use super::picker::{PendingRequest, PickedSongs, SharedPicker, lock};
use super::playlist_writes::exclusive_playlist_write;
use crate::ui::{
    AppWindow, Context, PlaylistPickerState, on_ui, playlist_detail, playlists, show_status,
};

enum RequestOutcome {
    Succeeded(String),
    Failed(String),
}

fn start_request(window: &AppWindow, shared: &SharedPicker) -> Option<PendingRequest> {
    let pending = lock(shared).start_request()?;
    window.global::<PlaylistPickerState>().set_is_busy(true);
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
    window.global::<PlaylistPickerState>().set_is_busy(false);
    match outcome {
        RequestOutcome::Succeeded(message) => {
            close(window, shared);
            show_status(window, &message);
        }
        RequestOutcome::Failed(message) => show_status(window, &message),
    }
}

pub fn pick(window: &AppWindow, context: &Context, shared: &SharedPicker, playlist_id: String) {
    let Some(server) = context.session.get() else {
        return;
    };
    let Some(PendingRequest { generation, songs }) = start_request(window, shared) else {
        return;
    };
    let picked = PickedPlaylist {
        name: lock(shared).playlist_name(&playlist_id),
        id: playlist_id,
    };
    match songs {
        PickedSongs::Ids(song_ids) => {
            let ids_request = IdsRequest {
                generation,
                song_ids,
            };
            pick_ids(window, context, shared, server, picked, ids_request);
        }
        PickedSongs::Listed(listed) => {
            close(window, shared);
            add_listed(window, context, server, picked, listed);
        }
    }
}

struct IdsRequest {
    generation: u64,
    song_ids: Vec<String>,
}

fn pick_ids(
    window: &AppWindow,
    context: &Context,
    shared: &SharedPicker,
    server: Arc<dyn MusicServer>,
    picked: PickedPlaylist,
    request: IdsRequest,
) {
    let (weak, shared) = (window.as_weak(), Arc::clone(shared));
    context.runtime.spawn(async move {
        let outcome = add_new_songs(server.as_ref(), &picked.id, &request.song_ids).await;
        on_ui(&weak, move |window| {
            finish_pick(window, &shared, request.generation, &picked, outcome);
        });
    });
}

async fn add_new_songs(
    server: &dyn MusicServer,
    playlist_id: &str,
    song_ids: &[String],
) -> feishin_api::Result<usize> {
    let _exclusive = exclusive_playlist_write(playlist_id).await;
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
    let has_changed = matches!(outcome, Ok(added) if added > 0);
    let outcome = match outcome {
        Ok(0) => RequestOutcome::Succeeded(tr!("Already in \"{}\"", name)),
        Ok(added) => RequestOutcome::Succeeded(tr_plural!(
            "Added {n} song to \"{}\"",
            "Added {n} songs to \"{}\"",
            added,
            name
        )),
        Err(error) => RequestOutcome::Failed(tr!("Could not add to \"{}\": {}", name, error)),
    };
    finish_request(window, shared, generation, outcome);
    if has_changed {
        playlists::refresh(window);
        playlist_detail::refresh_if_showing(window, &picked.id);
    }
}

pub fn create(window: &AppWindow, context: &Context, shared: &SharedPicker, name: &str) {
    let name = name.trim().to_owned();
    let Some(server) = context.session.get().filter(|_| !name.is_empty()) else {
        return;
    };
    let Some(PendingRequest { generation, songs }) = start_request(window, shared) else {
        return;
    };
    match songs {
        PickedSongs::Ids(song_ids) => {
            let ids_request = IdsRequest {
                generation,
                song_ids,
            };
            create_with_ids(window, context, shared, server, name, ids_request);
        }
        PickedSongs::Listed(listed) => {
            close(window, shared);
            create_listed(window, context, server, name, listed);
        }
    }
}

fn create_with_ids(
    window: &AppWindow,
    context: &Context,
    shared: &SharedPicker,
    server: Arc<dyn MusicServer>,
    name: String,
    request: IdsRequest,
) {
    let song_ids = new_song_ids(&[], &request.song_ids);
    let (weak, shared) = (window.as_weak(), Arc::clone(shared));
    context.runtime.spawn(async move {
        let result = server.create_playlist(&name, &song_ids).await;
        on_ui(&weak, move |window| {
            finish_create(window, &shared, request.generation, song_ids.len(), result);
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
    let is_created = result.is_ok();
    let outcome = match result {
        Ok(playlist) => RequestOutcome::Succeeded(tr_plural!(
            "Created \"{}\" with {n} song",
            "Created \"{}\" with {n} songs",
            song_count,
            playlist.name
        )),
        Err(error) => RequestOutcome::Failed(tr!("Could not create the playlist: {}", error)),
    };
    finish_request(window, shared, generation, outcome);
    if is_created {
        playlists::refresh(window);
    }
}
