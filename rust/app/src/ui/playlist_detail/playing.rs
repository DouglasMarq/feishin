use slint::ComponentHandle;

use super::binding::with_model;
use super::detail::{SharedDetail, lock};
use crate::controller::Command;
use crate::list_stream::{ListRow, Placement, PlaylistId, SongSource};
use crate::ui::{AppWindow, Context, PlayerState};

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
    fn shuffles(&self) -> bool {
        matches!(self, Self::FromTop { shuffle: true })
    }

    fn command(self, playlist_id: PlaylistId, song_count: usize) -> Command {
        let source = SongSource::Playlist(playlist_id);
        let every_row = 0..song_count;
        match self {
            Self::FromTop { .. } => Command::EnqueueFromList {
                source,
                ranges: vec![every_row],
                placement: Placement::Now,
            },
            Self::AtRow { index, song_id } => Command::PlayFromList {
                source,
                song_id,
                row_index: ListRow(index),
            },
        }
    }
}

pub fn play_playlist(window: &AppWindow, ctx: &Context, shared: &SharedDetail, start: PlayStart) {
    let playlist_id = lock(shared).id.clone();
    if playlist_id.is_empty() {
        return;
    }
    if start.shuffles() && !window.global::<PlayerState>().get_shuffle() {
        ctx.send(Command::ToggleShuffle);
    }
    let song_count = with_model(window, |model| model.total()).unwrap_or_default();
    ctx.send(start.command(PlaylistId(playlist_id), song_count));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playing_from_the_top_streams_every_row_and_rows_start_at_their_song() {
        let top = PlayStart::FromTop { shuffle: true };
        assert!(top.shuffles());
        assert!(matches!(
            top.command(PlaylistId("playlist-1".into()), 1200),
            Command::EnqueueFromList {
                source: SongSource::Playlist(PlaylistId(id)),
                ranges,
                placement: Placement::Now,
            } if id == "playlist-1" && ranges.len() == 1 && ranges.first() == Some(&(0..1200))
        ));
        let row = PlayStart::AtRow {
            index: 700,
            song_id: None,
        };
        assert!(!row.shuffles());
        assert!(matches!(
            row.command(PlaylistId("playlist-1".into()), 1200),
            Command::PlayFromList {
                song_id: None,
                row_index: ListRow(700),
                ..
            }
        ));
    }
}
