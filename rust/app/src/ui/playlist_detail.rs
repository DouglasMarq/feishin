use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use feishin_core::model::{Playlist, Song};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use super::playlists::{PlayMode, fetch_all, song_count_label};
use super::{
    Actions, AppState, AppWindow, Context, PlayerState, PlaylistDetailState, PlaylistSongRow,
    Route, format_duration, on_ui, show_status,
};
use crate::controller::Command;

const COVER_SIZE: u32 = 300;

/// Row selection with click, ctrl-click (toggle) and shift-click (range from the anchor).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Selection {
    selected: BTreeSet<usize>,
    anchor: Option<usize>,
}

impl Selection {
    pub fn click(&mut self, index: usize, toggle: bool, range: bool) {
        match self.anchor {
            Some(anchor) if range => {
                if !toggle {
                    self.selected.clear();
                }
                self.selected.extend(anchor.min(index)..=anchor.max(index));
            }
            _ if toggle => {
                if !self.selected.remove(&index) {
                    self.selected.insert(index);
                }
                self.anchor = Some(index);
            }
            _ => {
                self.selected.clear();
                self.selected.insert(index);
                self.anchor = Some(index);
            }
        }
    }

    pub fn select_all(&mut self, len: usize) {
        self.selected = (0..len).collect();
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn contains(&self, index: usize) -> bool {
        self.selected.contains(&index)
    }

    pub fn len(&self) -> usize {
        self.selected.len()
    }

    pub fn indexes(&self) -> Vec<usize> {
        self.selected.iter().copied().collect()
    }

    /// Rows a context menu on `index` acts on: the selection if it contains the row, else the row alone.
    pub fn targets(&self, index: usize) -> Vec<usize> {
        if self.contains(index) {
            self.indexes()
        } else {
            vec![index]
        }
    }
}

#[derive(Default)]
struct Detail {
    generation: u64,
    id: String,
    name: String,
    songs: Vec<Song>,
    selection: Selection,
    busy: bool,
}

type Shared = Arc<Mutex<Detail>>;

fn lock(shared: &Shared) -> MutexGuard<'_, Detail> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

pub fn header_info(songs: &[Song], owner: Option<&str>) -> String {
    let duration: u64 = songs.iter().map(|s| s.duration_ms).sum();
    let mut parts = vec![song_count_label(songs.len())];
    if !songs.is_empty() {
        parts.push(format_duration(duration));
    }
    parts.extend(owner.filter(|o| !o.is_empty()).map(|o| format!("by {o}")));
    parts.join(" • ")
}

fn song_row(song: &Song, index: usize) -> PlaylistSongRow {
    PlaylistSongRow {
        track: super::tracks::track_row(song, index),
        album_id: song.album_id.as_deref().unwrap_or_default().into(),
        artist_id: song
            .artists
            .first()
            .map(|a| a.id.as_str())
            .unwrap_or_default()
            .into(),
        selected: false,
    }
}

/// Applies a selection change to the rows that changed and to the selected count.
fn sync_selection(window: &AppWindow, before: &Selection, after: &Selection) {
    let state = window.global::<PlaylistDetailState>();
    let rows = state.get_rows();
    let changed = before
        .selected
        .symmetric_difference(&after.selected)
        .copied();
    for index in changed {
        if let Some(mut row) = rows.row_data(index) {
            row.selected = after.contains(index);
            rows.set_row_data(index, row);
        }
    }
    state.set_selected_count(super::clamp_i32(after.len() as u64));
}

fn update_selection(window: &AppWindow, shared: &Shared, f: impl FnOnce(&mut Selection, usize)) {
    let (before, after) = {
        let mut detail = lock(shared);
        let before = detail.selection.clone();
        let len = detail.songs.len();
        f(&mut detail.selection, len);
        (before, detail.selection.clone())
    };
    sync_selection(window, &before, &after);
}

fn reset_view(window: &AppWindow) {
    let state = window.global::<PlaylistDetailState>();
    state.set_name("".into());
    state.set_comment("".into());
    state.set_info("".into());
    state.set_has_cover(false);
    state.set_cover(slint::Image::default());
    state.set_rows(ModelRc::default());
    state.set_selected_count(0);
    state.set_busy(false);
}

fn load(window: &AppWindow, ctx: &Context, shared: &Shared, id: &str) {
    let generation = {
        let mut detail = lock(shared);
        *detail = Detail {
            generation: detail.generation + 1,
            id: id.to_owned(),
            ..Detail::default()
        };
        detail.generation
    };
    reset_view(window);
    let state = window.global::<PlaylistDetailState>();
    let Some(server) = ctx.session.get().filter(|_| !id.is_empty()) else {
        state.set_loading(false);
        return;
    };
    state.set_loading(true);
    let (weak, shared, ctx_task, id) = (
        window.as_weak(),
        Arc::clone(shared),
        ctx.clone(),
        id.to_owned(),
    );
    ctx.rt.spawn(async move {
        let (songs, playlists) =
            tokio::join!(server.playlist_songs(&id), fetch_all(server.as_ref()));
        let meta = playlists
            .ok()
            .and_then(|all| all.into_iter().find(|p| p.id == id));
        if let Some(cover_id) = meta.as_ref().and_then(|p| p.cover_art_id.clone()) {
            let (covers, server, weak, shared) = (
                ctx_task.covers.clone(),
                server.clone(),
                weak.clone(),
                Arc::clone(&shared),
            );
            ctx_task.rt.spawn(async move {
                let Ok(cover) = covers.cover(server.as_ref(), &cover_id, COVER_SIZE).await else {
                    return;
                };
                on_ui(&weak, move |window| {
                    if lock(&shared).generation != generation {
                        return;
                    }
                    if let Some(image) = super::player::to_image(&cover) {
                        let state = window.global::<PlaylistDetailState>();
                        state.set_cover(image);
                        state.set_has_cover(true);
                    }
                });
            });
        }
        on_ui(&weak, move |window| {
            apply_loaded(window, &shared, generation, songs, meta);
        });
    });
}

fn apply_loaded(
    window: &AppWindow,
    shared: &Shared,
    generation: u64,
    songs: feishin_api::Result<Vec<Song>>,
    meta: Option<Playlist>,
) {
    let mut detail = lock(shared);
    if detail.generation != generation {
        return;
    }
    let state = window.global::<PlaylistDetailState>();
    state.set_loading(false);
    let songs = match songs {
        Ok(songs) => songs,
        Err(e) => {
            drop(detail);
            show_status(window, &format!("Could not load the playlist: {e}"));
            return;
        }
    };
    detail.name = meta
        .as_ref()
        .map_or_else(|| "Playlist".to_owned(), |p| p.name.clone());
    state.set_name(detail.name.as_str().into());
    state.set_comment(
        meta.as_ref()
            .and_then(|p| p.comment.as_deref())
            .unwrap_or_default()
            .trim()
            .into(),
    );
    state.set_info(header_info(&songs, meta.as_ref().and_then(|p| p.owner.as_deref())).into());
    let rows: Vec<PlaylistSongRow> = songs
        .iter()
        .enumerate()
        .map(|(i, song)| song_row(song, i))
        .collect();
    state.set_rows(ModelRc::new(VecModel::from(rows)));
    detail.songs = songs;
}

/// Reloads the detail view if it currently shows `playlist_id`.
pub fn refresh_if_showing(window: &AppWindow, playlist_id: &str) {
    let app = window.global::<AppState>();
    if app.get_route() == Route::PlaylistDetail && app.get_detail_id() == playlist_id {
        window
            .global::<PlaylistDetailState>()
            .invoke_load(playlist_id.into());
    }
}

fn songs_at(detail: &Detail, indexes: &[usize]) -> Vec<Song> {
    indexes
        .iter()
        .filter_map(|&i| detail.songs.get(i).cloned())
        .collect()
}

fn remove(window: &AppWindow, ctx: &Context, shared: &Shared, indexes: Vec<usize>) {
    let Some(server) = ctx.session.get() else {
        return;
    };
    let (generation, id, name) = {
        let mut detail = lock(shared);
        if detail.busy || indexes.is_empty() || detail.id.is_empty() {
            return;
        }
        detail.busy = true;
        (detail.generation, detail.id.clone(), detail.name.clone())
    };
    window.global::<PlaylistDetailState>().set_busy(true);
    let (weak, shared) = (window.as_weak(), Arc::clone(shared));
    ctx.rt.spawn(async move {
        let result = server.remove_from_playlist(&id, &indexes).await;
        on_ui(&weak, move |window| {
            {
                let mut detail = lock(&shared);
                if detail.generation != generation {
                    return;
                }
                detail.busy = false;
            }
            window.global::<PlaylistDetailState>().set_busy(false);
            match result {
                Ok(()) => {
                    let what = song_count_label(indexes.len());
                    show_status(window, &format!("Removed {what} from \"{name}\""));
                    refresh_if_showing(window, &id);
                }
                Err(e) => show_status(window, &format!("Could not remove songs: {e}")),
            }
        });
    });
}

fn add_to_playlist(window: &AppWindow, songs: &[Song]) {
    if songs.is_empty() {
        return;
    }
    let ids: Vec<SharedString> = songs.iter().map(|s| s.id.as_str().into()).collect();
    window
        .global::<Actions>()
        .invoke_add_to_playlist(ModelRc::new(VecModel::from(ids)));
}

pub fn install(window: &AppWindow, ctx: &Context) {
    let shared: Shared = Arc::default();
    let state = window.global::<PlaylistDetailState>();

    let (weak, load_ctx, load_shared) = (window.as_weak(), ctx.clone(), Arc::clone(&shared));
    state.on_load(move |id| {
        if let Some(window) = weak.upgrade() {
            load(&window, &load_ctx, &load_shared, &id);
        }
    });

    let (weak, play_ctx, play_shared) = (window.as_weak(), ctx.clone(), Arc::clone(&shared));
    state.on_play(move |shuffle| {
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

    let (row_ctx, row_shared) = (ctx.clone(), Arc::clone(&shared));
    state.on_play_row(move |index| {
        let detail = lock(&row_shared);
        let Some(song) = usize::try_from(index)
            .ok()
            .and_then(|i| detail.songs.get(i))
        else {
            return;
        };
        row_ctx.send(Command::PlaySongs {
            song_id: Some(song.id.clone()),
            songs: detail.songs.clone(),
        });
    });

    let (weak, select_shared) = (window.as_weak(), Arc::clone(&shared));
    state.on_select(move |index, toggle, range| {
        let (Some(window), Ok(index)) = (weak.upgrade(), usize::try_from(index)) else {
            return;
        };
        update_selection(&window, &select_shared, |selection, len| {
            if index < len {
                selection.click(index, toggle, range);
            }
        });
    });

    let (weak, all_shared) = (window.as_weak(), Arc::clone(&shared));
    state.on_select_all(move || {
        if let Some(window) = weak.upgrade() {
            update_selection(&window, &all_shared, Selection::select_all);
        }
    });

    let (weak, clear_shared) = (window.as_weak(), Arc::clone(&shared));
    state.on_clear_selection(move || {
        if let Some(window) = weak.upgrade() {
            update_selection(&window, &clear_shared, |selection, _| selection.clear());
        }
    });

    let (weak, remove_ctx, remove_shared) = (window.as_weak(), ctx.clone(), Arc::clone(&shared));
    state.on_remove_selected(move || {
        if let Some(window) = weak.upgrade() {
            let indexes = lock(&remove_shared).selection.indexes();
            remove(&window, &remove_ctx, &remove_shared, indexes);
        }
    });

    let (weak, add_shared) = (window.as_weak(), Arc::clone(&shared));
    state.on_add_selected_to_playlist(move || {
        if let Some(window) = weak.upgrade() {
            let songs = {
                let detail = lock(&add_shared);
                songs_at(&detail, &detail.selection.indexes())
            };
            add_to_playlist(&window, &songs);
        }
    });

    let (weak, menu_ctx, menu_shared) = (window.as_weak(), ctx.clone(), Arc::clone(&shared));
    state.on_context_action(move |index, action| {
        let (Some(window), Ok(index)) = (weak.upgrade(), usize::try_from(index)) else {
            return;
        };
        let (targets, songs) = {
            let detail = lock(&menu_shared);
            let targets = detail.selection.targets(index);
            let songs = songs_at(&detail, &targets);
            (targets, songs)
        };
        if songs.is_empty() {
            return;
        }
        match action.as_str() {
            "playlist" => add_to_playlist(&window, &songs),
            "remove" => remove(&window, &menu_ctx, &menu_shared, targets),
            how => menu_ctx.send(PlayMode::parse(how).command(songs)),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::test_song;

    #[test]
    fn clicks_select_toggle_and_extend() {
        let mut selection = Selection::default();
        selection.click(2, false, false);
        assert_eq!(selection.indexes(), [2]);
        selection.click(5, false, true);
        assert_eq!(selection.indexes(), [2, 3, 4, 5]);
        selection.click(0, false, true);
        assert_eq!(selection.indexes(), [0, 1, 2]);
        selection.click(7, true, false);
        assert_eq!(selection.indexes(), [0, 1, 2, 7]);
        selection.click(1, true, false);
        assert_eq!(selection.indexes(), [0, 2, 7]);
        selection.click(3, true, true);
        assert_eq!(selection.indexes(), [0, 1, 2, 3, 7]);
        selection.click(4, false, false);
        assert_eq!(selection.indexes(), [4]);
        selection.select_all(3);
        assert_eq!(selection.len(), 3);
        selection.clear();
        assert_eq!(selection.len(), 0);
    }

    #[test]
    fn context_targets_follow_the_selection() {
        let mut selection = Selection::default();
        selection.click(1, false, false);
        selection.click(3, true, false);
        assert_eq!(selection.targets(3), [1, 3]);
        assert_eq!(selection.targets(2), [2]);
    }

    #[test]
    fn header_info_sums_songs() {
        let songs = [test_song("a", 60_000), test_song("b", 90_000)];
        assert_eq!(
            header_info(&songs, Some("demo")),
            "2 songs • 2:30 • by demo"
        );
        assert_eq!(header_info(&[], None), "0 songs");
    }

    #[test]
    fn song_rows_link_album_and_artist() {
        let server = crate::demo::FakeServer::new(50);
        let row = song_row(&server.song_at(13), 0);
        assert_eq!(row.album_id, "album-1");
        assert!(row.artist_id.starts_with("artist-"));
        assert_eq!(row.track.index, 1);
        assert!(!row.selected);
    }
}
