use std::collections::HashSet;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use feishin_core::model::{Playlist, Song};
use slint::{ComponentHandle, Model, ModelRc, VecModel};

use super::playlists::{fetch_all, song_count_label};
use super::{
    Actions, AppWindow, Context, PickerRow, PlaylistPickerState, format_duration, on_ui,
    playlist_detail, playlists, show_status,
};

#[derive(Default)]
struct Picker {
    generation: u64,
    song_ids: Vec<String>,
    playlists: Vec<Playlist>,
    busy: bool,
}

type Shared = Arc<Mutex<Picker>>;

fn lock(shared: &Shared) -> MutexGuard<'_, Picker> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Playlists whose name contains `filter`, ignoring case.
pub fn filter_playlists<'a>(playlists: &'a [Playlist], filter: &str) -> Vec<&'a Playlist> {
    let filter = filter.trim().to_lowercase();
    playlists
        .iter()
        .filter(|p| filter.is_empty() || p.name.to_lowercase().contains(&filter))
        .collect()
}

/// `song_ids` minus the ones already in the playlist and repeated ones, keeping their order.
pub fn new_song_ids(existing: &[Song], song_ids: &[String]) -> Vec<String> {
    let mut seen: HashSet<&str> = existing.iter().map(|s| s.id.as_str()).collect();
    song_ids
        .iter()
        .filter(|id| seen.insert(id.as_str()))
        .cloned()
        .collect()
}

fn picker_row(playlist: &Playlist) -> PickerRow {
    PickerRow {
        id: playlist.id.as_str().into(),
        name: playlist.name.as_str().into(),
        info: format!(
            "{} • {}",
            song_count_label(playlist.song_count as usize),
            format_duration(playlist.duration_ms)
        )
        .into(),
    }
}

fn show_rows(window: &AppWindow, shared: &Shared) {
    let state = window.global::<PlaylistPickerState>();
    let rows: Vec<PickerRow> = filter_playlists(&lock(shared).playlists, &state.get_filter())
        .into_iter()
        .map(picker_row)
        .collect();
    state.set_rows(ModelRc::new(VecModel::from(rows)));
}

fn close(window: &AppWindow, shared: &Shared) {
    {
        let mut picker = lock(shared);
        *picker = Picker {
            generation: picker.generation + 1,
            ..Picker::default()
        };
    }
    let state = window.global::<PlaylistPickerState>();
    state.set_open(false);
    state.set_creating(false);
    state.set_busy(false);
    state.set_rows(ModelRc::default());
}

fn open(window: &AppWindow, ctx: &Context, shared: &Shared, song_ids: Vec<String>) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    if song_ids.is_empty() {
        return;
    }
    let state = window.global::<PlaylistPickerState>();
    let count = song_ids.len();
    let generation = {
        let mut picker = lock(shared);
        *picker = Picker {
            generation: picker.generation + 1,
            song_ids,
            ..Picker::default()
        };
        picker.generation
    };
    state.set_song_count(super::clamp_i32(count as u64));
    state.set_filter("".into());
    state.set_rows(ModelRc::default());
    state.set_creating(false);
    state.set_busy(false);
    state.set_loading(true);
    state.set_open(true);
    let (weak, shared) = (window.as_weak(), Arc::clone(shared));
    ctx.rt.spawn(async move {
        let result = fetch_all(server.as_ref()).await;
        on_ui(&weak, move |window| {
            {
                let mut picker = lock(&shared);
                if picker.generation != generation {
                    return;
                }
                match result {
                    Ok(playlists) => picker.playlists = playlists,
                    Err(e) => {
                        drop(picker);
                        show_status(window, &format!("Could not load playlists: {e}"));
                    }
                }
            }
            window.global::<PlaylistPickerState>().set_loading(false);
            show_rows(window, &shared);
        });
    });
}

/// Marks the picker busy and returns what the request needs, or `None` if a request is running.
fn start_request(window: &AppWindow, shared: &Shared) -> Option<(u64, Vec<String>)> {
    let mut picker = lock(shared);
    if picker.busy || picker.song_ids.is_empty() {
        return None;
    }
    picker.busy = true;
    window.global::<PlaylistPickerState>().set_busy(true);
    Some((picker.generation, picker.song_ids.clone()))
}

/// Ends a request: closes the picker on success, keeps it open on error so the user can retry.
fn finish_request(
    window: &AppWindow,
    shared: &Shared,
    generation: u64,
    outcome: Result<String, String>,
) {
    {
        let mut picker = lock(shared);
        if picker.generation != generation {
            return;
        }
        picker.busy = false;
    }
    window.global::<PlaylistPickerState>().set_busy(false);
    match outcome {
        Ok(message) => {
            close(window, shared);
            show_status(window, &message);
        }
        Err(message) => show_status(window, &message),
    }
}

fn pick(window: &AppWindow, ctx: &Context, shared: &Shared, playlist_id: String) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let Some((generation, song_ids)) = start_request(window, shared) else {
        return;
    };
    let name = lock(shared)
        .playlists
        .iter()
        .find(|p| p.id == playlist_id)
        .map_or_else(|| "playlist".to_owned(), |p| p.name.clone());
    let (weak, shared) = (window.as_weak(), Arc::clone(shared));
    ctx.rt.spawn(async move {
        let outcome = async {
            let existing = server.playlist_songs(&playlist_id).await?;
            let ids = new_song_ids(&existing, &song_ids);
            if !ids.is_empty() {
                server.add_to_playlist(&playlist_id, &ids).await?;
            }
            Ok::<_, feishin_api::ApiError>(ids.len())
        }
        .await;
        on_ui(&weak, move |window| {
            let changed = matches!(outcome, Ok(added) if added > 0);
            let outcome = match outcome {
                Ok(0) => Ok(format!("Already in \"{name}\"")),
                Ok(added) => Ok(format!("Added {} to \"{name}\"", song_count_label(added))),
                Err(e) => Err(format!("Could not add to \"{name}\": {e}")),
            };
            finish_request(window, &shared, generation, outcome);
            if changed {
                playlists::refresh(window);
                playlist_detail::refresh_if_showing(window, &playlist_id);
            }
        });
    });
}

fn create(window: &AppWindow, ctx: &Context, shared: &Shared, name: String) {
    let name = name.trim().to_owned();
    let Some(server) = ctx.session.get().filter(|_| !name.is_empty()) else {
        return;
    };
    let Some((generation, song_ids)) = start_request(window, shared) else {
        return;
    };
    let song_ids = new_song_ids(&[], &song_ids);
    let (weak, shared) = (window.as_weak(), Arc::clone(shared));
    ctx.rt.spawn(async move {
        let result = server.create_playlist(&name, &song_ids).await;
        on_ui(&weak, move |window| {
            let created = result.is_ok();
            let outcome = result
                .map(|playlist| {
                    format!(
                        "Created \"{}\" with {}",
                        playlist.name,
                        song_count_label(song_ids.len())
                    )
                })
                .map_err(|e| format!("Could not create the playlist: {e}"));
            finish_request(window, &shared, generation, outcome);
            if created {
                playlists::refresh(window);
            }
        });
    });
}

pub fn install(window: &AppWindow, ctx: &Context) {
    let shared: Shared = Arc::default();
    let state = window.global::<PlaylistPickerState>();

    let (weak, open_ctx, open_shared) = (window.as_weak(), ctx.clone(), Arc::clone(&shared));
    window.global::<Actions>().on_add_to_playlist(move |ids| {
        if let Some(window) = weak.upgrade() {
            let ids = ids.iter().map(|id| id.to_string()).collect();
            open(&window, &open_ctx, &open_shared, ids);
        }
    });

    let (weak, filter_shared) = (window.as_weak(), Arc::clone(&shared));
    state.on_filter_edited(move |_| {
        if let Some(window) = weak.upgrade() {
            show_rows(&window, &filter_shared);
        }
    });

    let (weak, pick_ctx, pick_shared) = (window.as_weak(), ctx.clone(), Arc::clone(&shared));
    state.on_pick(move |id| {
        if let Some(window) = weak.upgrade() {
            pick(&window, &pick_ctx, &pick_shared, id.to_string());
        }
    });

    let (weak, create_ctx, create_shared) = (window.as_weak(), ctx.clone(), Arc::clone(&shared));
    state.on_create(move |name| {
        if let Some(window) = weak.upgrade() {
            create(&window, &create_ctx, &create_shared, name.to_string());
        }
    });

    let (weak, cancel_shared) = (window.as_weak(), Arc::clone(&shared));
    state.on_cancel(move || {
        if let Some(window) = weak.upgrade() {
            close(&window, &cancel_shared);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::{FakeServer, test_song};
    use feishin_api::MusicServer;

    #[tokio::test]
    async fn filter_ignores_case_and_whitespace() {
        let server = FakeServer::new(10);
        let playlists = fetch_all(&server).await.unwrap_or_default();
        assert_eq!(filter_playlists(&playlists, "").len(), playlists.len());
        let names: Vec<&str> = filter_playlists(&playlists, "  WORK ")
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(names, ["Workout"]);
        assert!(filter_playlists(&playlists, "nothing like this").is_empty());
    }

    #[test]
    fn duplicates_are_skipped_in_order() {
        let existing = [test_song("song-1", 1), test_song("song-2", 1)];
        let ids: Vec<String> = ["song-3", "song-1", "song-4", "song-3"]
            .map(String::from)
            .to_vec();
        assert_eq!(new_song_ids(&existing, &ids), ["song-3", "song-4"]);
        assert_eq!(new_song_ids(&[], &ids), ["song-3", "song-1", "song-4"]);
    }

    #[tokio::test]
    async fn demo_playlists_can_be_edited() {
        let server = FakeServer::new(100);
        let created = server
            .create_playlist("Mine", &["song-1".into(), "song-2".into()])
            .await;
        let Ok(created) = created else {
            panic!("create failed");
        };
        assert_eq!(created.song_count, 2);
        let added = server
            .add_to_playlist(&created.id, &["song-3".into()])
            .await;
        assert!(added.is_ok());
        assert!(server.remove_from_playlist(&created.id, &[0]).await.is_ok());
        let ids: Vec<String> = server
            .playlist_songs(&created.id)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert_eq!(ids, ["song-2", "song-3"]);
        assert!(server.delete_playlist(&created.id).await.is_ok());
        assert!(server.playlist_songs(&created.id).await.is_err());
    }
}
