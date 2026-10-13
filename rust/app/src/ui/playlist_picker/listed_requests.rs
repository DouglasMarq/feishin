use std::collections::HashSet;
use std::sync::Arc;

use feishin_api::MusicServer;
use slint::ComponentHandle;

use super::streamed::{ListedAdd, add_listed_songs, existing_song_ids};
use crate::ui::playlists::song_count_label;
use crate::ui::tracks::ListedSongs;
use crate::ui::{AppWindow, Context, on_ui, playlist_detail, playlists, show_status};

pub struct PickedPlaylist {
    pub id: String,
    pub name: String,
}

fn progress_reporter(
    window: &AppWindow,
    name: &str,
    total: usize,
) -> impl Fn(usize) + Send + 'static {
    let (weak, name) = (window.as_weak(), name.to_owned());
    move |read| {
        let message = format!("Adding to \"{name}\": {read} of {total} songs");
        on_ui(&weak, move |window| show_status(window, &message));
    }
}

pub fn add_listed(
    window: &AppWindow,
    ctx: &Context,
    server: Arc<dyn MusicServer>,
    picked: PickedPlaylist,
    listed: ListedSongs,
) {
    let report = progress_reporter(window, &picked.name, listed.song_count());
    report(0);
    let weak = window.as_weak();
    ctx.rt.spawn(async move {
        let outcome = match existing_song_ids(server.as_ref(), &picked.id).await {
            Ok(existing) => {
                add_listed_songs(server.as_ref(), &picked.id, listed, existing, report).await
            }
            Err(error) => ListedAdd::failed(error),
        };
        on_ui(&weak, move |window| finish_add(window, &picked, outcome));
    });
}

fn finish_add(window: &AppWindow, picked: &PickedPlaylist, outcome: ListedAdd) {
    let name = &picked.name;
    let added = song_count_label(outcome.added);
    let message = match (outcome.failure, outcome.added) {
        (None, 0) => format!("Already in \"{name}\""),
        (None, _) => format!("Added {added} to \"{name}\""),
        (Some(error), 0) => format!("Could not add to \"{name}\": {error}"),
        (Some(error), _) => format!("Added only {added} to \"{name}\": {error}"),
    };
    show_status(window, &message);
    if outcome.added > 0 {
        playlists::refresh(window);
        playlist_detail::refresh_if_showing(window, &picked.id);
    }
}

pub fn create_listed(
    window: &AppWindow,
    ctx: &Context,
    server: Arc<dyn MusicServer>,
    name: String,
    listed: ListedSongs,
) {
    let report = progress_reporter(window, &name, listed.song_count());
    report(0);
    let weak = window.as_weak();
    ctx.rt.spawn(async move {
        let created = server.create_playlist(&name, &[]).await;
        let outcome = match &created {
            Ok(playlist) => {
                add_listed_songs(
                    server.as_ref(),
                    &playlist.id,
                    listed,
                    HashSet::new(),
                    report,
                )
                .await
            }
            Err(_) => ListedAdd::default(),
        };
        on_ui(&weak, move |window| {
            finish_create(window, created.map(|playlist| playlist.name), outcome);
        });
    });
}

fn finish_create(window: &AppWindow, created: feishin_api::Result<String>, outcome: ListedAdd) {
    let added = song_count_label(outcome.added);
    let message = match (created, outcome.failure) {
        (Err(error), _) => format!("Could not create the playlist: {error}"),
        (Ok(name), None) => format!("Created \"{name}\" with {added}"),
        (Ok(name), Some(error)) => format!("Created \"{name}\" with only {added}: {error}"),
    };
    show_status(window, &message);
    playlists::refresh(window);
}
