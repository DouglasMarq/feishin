use feishin_core::model::Song;

use super::binding::with_model;
use super::editing::add_to_playlist;
use super::model::PlaylistSongsModel;
use crate::controller::Command;
use crate::list_stream::{Placement, PlaylistId, RowRanges, SongSource};
use crate::ui::playlist_picker::PlaylistPicker;
use crate::ui::{AppWindow, Context, PlaylistSongAction};

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

impl SongAction {
    fn placement(self) -> Option<Placement> {
        match self {
            Self::Play => Some(Placement::Now),
            Self::PlayNext => Some(Placement::Next),
            Self::AddToQueue => Some(Placement::Last),
            Self::AddToPlaylist => None,
        }
    }
}

pub fn act_on_rows(
    window: &AppWindow,
    ctx: &Context,
    picker: &PlaylistPicker,
    indexes: &[usize],
    action: SongAction,
) {
    match action.placement() {
        Some(placement) => enqueue_rows(window, ctx, indexes, placement),
        None => add_rows_to_playlist(window, picker, indexes),
    }
}

fn enqueue_rows(window: &AppWindow, ctx: &Context, indexes: &[usize], placement: Placement) {
    let Some(playlist_id) = with_model(window, PlaylistSongsModel::playlist_id) else {
        return;
    };
    if playlist_id.is_empty() || indexes.is_empty() {
        return;
    }
    ctx.send(Command::EnqueueFromList {
        source: SongSource::Playlist(PlaylistId(playlist_id)),
        ranges: RowRanges::covering(indexes),
        placement,
    });
}

fn add_rows_to_playlist(window: &AppWindow, picker: &PlaylistPicker, indexes: &[usize]) {
    let Some(snapshot) = with_model(window, |model| model.snapshot(indexes)) else {
        return;
    };
    match snapshot.is_loaded() {
        true => add_songs_to_playlist(window, &snapshot.into_loaded()),
        false => picker.open_listed(window, snapshot.into_listed()),
    }
}

fn add_songs_to_playlist(window: &AppWindow, songs: &[Song]) {
    if !songs.is_empty() {
        add_to_playlist(window, songs);
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
