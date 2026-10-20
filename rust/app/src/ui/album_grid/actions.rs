use feishin_api::MusicServer;
use feishin_core::model::Song;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use super::super::{Actions, AppWindow, Context, on_ui, show_status};
use crate::controller::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlbumAction {
    Play,
    PlayNext,
    AddToQueue,
    AddToPlaylist,
}

async fn album_songs(server: &dyn MusicServer, album_id: &str) -> anyhow::Result<Vec<Song>> {
    Ok(server.album(album_id).await?.songs)
}

fn offer_songs_for_playlist(window: &AppWindow, songs: &[Song]) {
    let song_ids: Vec<SharedString> = songs.iter().map(|song| song.id.as_str().into()).collect();
    window
        .global::<Actions>()
        .invoke_add_to_playlist(ModelRc::new(VecModel::from(song_ids)));
}

fn run_action(
    context: &Context,
    weak_window: &slint::Weak<AppWindow>,
    songs: Vec<Song>,
    action: AlbumAction,
) {
    match action {
        AlbumAction::Play => context.send(Command::PlaySongs {
            songs,
            song_id: None,
        }),
        AlbumAction::PlayNext => context.send(Command::AddNext(songs)),
        AlbumAction::AddToQueue => context.send(Command::AddLast(songs)),
        AlbumAction::AddToPlaylist => on_ui(weak_window, move |window| {
            offer_songs_for_playlist(window, &songs);
        }),
    }
}

pub fn album_action(window: &AppWindow, context: &Context, album_id: String, action: AlbumAction) {
    let Some(server) = context.session.get() else {
        return;
    };
    let weak_window = window.as_weak();
    let context = context.clone();
    context.runtime.clone().spawn(async move {
        match album_songs(server.as_ref(), &album_id).await {
            Ok(songs) => run_action(&context, &weak_window, songs, action),
            Err(error) => on_ui(&weak_window, move |window| {
                show_status(
                    window,
                    &tr!("Could not load the album: {}", format!("{error:#}")),
                );
            }),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::FakeServer;

    #[tokio::test]
    async fn album_songs_come_from_the_server() {
        let server = FakeServer::new(48);
        let songs = album_songs(&server, "album-1").await.unwrap_or_default();
        assert_eq!(songs.len(), 12);
        assert_eq!(songs[0].album_id.as_deref(), Some("album-1"));
    }
}
