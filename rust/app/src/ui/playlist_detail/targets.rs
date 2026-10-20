use feishin_core::model::Song;

use crate::list_stream::{PlaylistId, RowRanges, SongSource};
use crate::ui::tracks::{ListedSongs, SelectionRanges};

pub struct TargetSnapshot {
    pub playlist_id: String,
    pub rows: Vec<(usize, Option<Song>)>,
}

impl TargetSnapshot {
    pub fn is_loaded(&self) -> bool {
        self.rows.iter().all(|(_, song)| song.is_some())
    }

    pub fn into_loaded(self) -> Vec<Song> {
        self.rows.into_iter().filter_map(|(_, song)| song).collect()
    }

    pub fn into_listed(self) -> ListedSongs {
        let indexes: Vec<usize> = self.rows.iter().map(|(index, _)| *index).collect();
        ListedSongs {
            source: SongSource::Playlist(PlaylistId(self.playlist_id)),
            ranges: SelectionRanges::from_ranges(RowRanges::covering(&indexes)),
        }
    }
}
