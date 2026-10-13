use feishin_core::model::FolderId;
use slint::ComponentHandle;

use super::binding::with_model;
use crate::controller::Command;
use crate::list_stream::{ListRow, Placement, RowRanges, SongSource};
use crate::ui::{AppWindow, Context, FolderPlayMode, FolderSongAction, PlayerState};

pub fn play_folder(window: &AppWindow, ctx: &Context, folder_id: &str, mode: FolderPlayMode) {
    let source = SongSource::FolderTree(FolderId::from_detail_id(folder_id));
    let placement = match mode {
        FolderPlayMode::Now => {
            ctx.send(Command::PlayFromList {
                source,
                song_id: None,
                row_index: ListRow(0),
            });
            return;
        }
        FolderPlayMode::Shuffle => {
            if !window.global::<PlayerState>().get_shuffle() {
                ctx.send(Command::ToggleShuffle);
            }
            Placement::Now
        }
        FolderPlayMode::Next => Placement::Next,
        FolderPlayMode::Last => Placement::Last,
    };
    ctx.send(Command::EnqueueFromList {
        source,
        ranges: vec![RowRanges::WHOLE_LIST],
        placement,
    });
}

pub fn play_row(window: &AppWindow, ctx: &Context, row: usize) {
    let Some((folder, (position, song))) =
        with_model(window, |model| Some((model.folder(), model.song_at(row)?))).flatten()
    else {
        return;
    };
    ctx.send(Command::PlayFromList {
        source: SongSource::Folder(folder),
        song_id: Some(song.id),
        row_index: ListRow(position),
    });
}

pub fn act_on_song(window: &AppWindow, ctx: &Context, row: usize, action: FolderSongAction) {
    let Some((_, song)) = with_model(window, |model| model.song_at(row)).flatten() else {
        return;
    };
    ctx.send(match action {
        FolderSongAction::PlayNext => Command::AddNext(vec![song]),
        FolderSongAction::AddToQueue => Command::AddLast(vec![song]),
    });
}
