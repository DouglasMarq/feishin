use feishin_core::model::FolderId;
use slint::ComponentHandle;

use super::binding::with_model;
use crate::controller::Command;
use crate::list_stream::{ListRow, Placement, RowRanges, SongSource};
use crate::ui::{AppWindow, Context, FolderPlayMode, FolderSongAction, PlayerState};

pub fn play_folder(window: &AppWindow, context: &Context, folder_id: &str, mode: FolderPlayMode) {
    let source = SongSource::FolderTree(FolderId::from_detail_id(folder_id));
    let placement = match mode {
        FolderPlayMode::Now => {
            context.send(Command::PlayFromList {
                source,
                song_id: None,
                row_index: ListRow(0),
            });
            return;
        }
        FolderPlayMode::Shuffle => {
            if !window.global::<PlayerState>().get_is_shuffled() {
                context.send(Command::ToggleShuffle);
            }
            Placement::Now
        }
        FolderPlayMode::Next => Placement::Next,
        FolderPlayMode::Last => Placement::Last,
    };
    context.send(Command::EnqueueFromList {
        source,
        ranges: vec![RowRanges::WHOLE_LIST],
        placement,
    });
}

pub fn play_row(window: &AppWindow, context: &Context, row: usize) {
    let Some((folder, (position, song))) =
        with_model(window, |model| Some((model.folder(), model.song_at(row)?))).flatten()
    else {
        return;
    };
    context.send(Command::PlayFromList {
        source: SongSource::Folder(folder),
        song_id: Some(song.id),
        row_index: ListRow(position),
    });
}

pub fn act_on_song(window: &AppWindow, context: &Context, row: usize, action: FolderSongAction) {
    let Some((_, song)) = with_model(window, |model| model.song_at(row)).flatten() else {
        return;
    };
    context.send(match action {
        FolderSongAction::PlayNext => Command::AddNext(vec![song]),
        FolderSongAction::AddToQueue => Command::AddLast(vec![song]),
    });
}
