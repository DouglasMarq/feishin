use feishin_core::model::Song;
use slint::ComponentHandle;

use super::binding::with_model;
use super::editing::add_to_playlist;
use crate::ui::playlists::PlayMode;
use crate::ui::{AppWindow, Context, PlaylistSongAction, on_ui, show_status};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SongAction {
    Play,
    PlayNext,
    AddToQueue,
    AddToPlaylist,
}

impl SongAction {
    pub fn from_menu(action: PlaylistSongAction) -> Option<Self> {
        match action {
            PlaylistSongAction::Play => Some(Self::Play),
            PlaylistSongAction::PlayNext => Some(Self::PlayNext),
            PlaylistSongAction::AddToQueue => Some(Self::AddToQueue),
            PlaylistSongAction::AddToPlaylist => Some(Self::AddToPlaylist),
            PlaylistSongAction::Remove => None,
        }
    }
}

pub fn act_on_rows(window: &AppWindow, ctx: &Context, indexes: &[usize], action: SongAction) {
    let Some(snapshot) = with_model(window, |model| model.snapshot(indexes)) else {
        return;
    };
    if snapshot.is_loaded() {
        perform(window, ctx, action, snapshot.into_loaded());
        return;
    }
    let Some(server) = ctx.session.get() else {
        return;
    };
    let (weak, action_ctx) = (window.as_weak(), ctx.clone());
    ctx.rt.spawn(async move {
        let result = snapshot.resolve(server.as_ref()).await;
        on_ui(&weak, move |window| match result {
            Ok(songs) => perform(window, &action_ctx, action, songs),
            Err(error) => show_status(window, &format!("Could not load the selection: {error}")),
        });
    });
}

fn perform(window: &AppWindow, ctx: &Context, action: SongAction, songs: Vec<Song>) {
    if songs.is_empty() {
        return;
    }
    match action {
        SongAction::AddToPlaylist => add_to_playlist(window, &songs),
        SongAction::Play => ctx.send(PlayMode::Now.command(songs)),
        SongAction::PlayNext => ctx.send(PlayMode::Next.command(songs)),
        SongAction::AddToQueue => ctx.send(PlayMode::Last.command(songs)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_actions_map_to_song_actions() {
        assert_eq!(
            SongAction::from_menu(PlaylistSongAction::PlayNext),
            Some(SongAction::PlayNext)
        );
        assert_eq!(
            SongAction::from_menu(PlaylistSongAction::AddToPlaylist),
            Some(SongAction::AddToPlaylist)
        );
        assert_eq!(SongAction::from_menu(PlaylistSongAction::Remove), None);
    }
}
