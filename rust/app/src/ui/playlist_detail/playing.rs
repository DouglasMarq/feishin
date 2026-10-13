use std::sync::Arc;

use feishin_core::model::Song;
use slint::ComponentHandle;

use super::detail::{PlayTarget, SharedDetail, lock};
use super::fetch_all::fetch_all_playlist_songs;
use crate::controller::Command;
use crate::ui::{AppWindow, Context, PlayerState, on_ui, show_status};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayStart {
    FromTop {
        shuffle: bool,
    },
    AtRow {
        index: usize,
        song_id: Option<String>,
    },
}

impl PlayStart {
    fn song_id_in(self, songs: &[Song]) -> Option<String> {
        match self {
            Self::FromTop { .. } => None,
            Self::AtRow { index, song_id } => {
                song_id.or_else(|| songs.get(index).map(|song| song.id.clone()))
            }
        }
    }
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
    let shuffle_requested = matches!(start, PlayStart::FromTop { shuffle: true });
    if shuffle_requested && !window.global::<PlayerState>().get_shuffle() {
        ctx.send(Command::ToggleShuffle);
    }
    let song_id = start.song_id_in(&songs);
    ctx.send(Command::PlaySongs { songs, song_id });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::test_song;

    #[test]
    fn rows_start_at_their_song_even_before_their_page_loaded() {
        let songs = vec![test_song("first", 1000), test_song("second", 1000)];
        let unloaded = PlayStart::AtRow {
            index: 1,
            song_id: None,
        };
        assert_eq!(unloaded.song_id_in(&songs).as_deref(), Some("second"));
        let loaded = PlayStart::AtRow {
            index: 1,
            song_id: Some("chosen".into()),
        };
        assert_eq!(loaded.song_id_in(&songs).as_deref(), Some("chosen"));
        let top = PlayStart::FromTop { shuffle: true };
        assert_eq!(top.song_id_in(&songs), None);
    }
}
