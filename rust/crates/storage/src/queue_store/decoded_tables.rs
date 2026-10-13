use std::sync::Arc;

use feishin_core::model::{ArtistRef, SongId};
use feishin_core::queue::{QueueItem, QueueTrack};

use super::compact::CompactTrack;

#[derive(Debug, thiserror::Error)]
#[error("a track refers to a missing shared value")]
pub(super) struct DanglingReference;

pub(super) struct DecodedTables {
    strings: Vec<Arc<str>>,
    artist_lists: Vec<Arc<[ArtistRef]>>,
}

impl DecodedTables {
    pub(super) fn new(strings: Vec<String>, artist_lists: Vec<Vec<ArtistRef>>) -> Self {
        Self {
            strings: strings.into_iter().map(Arc::from).collect(),
            artist_lists: artist_lists.into_iter().map(Arc::from).collect(),
        }
    }

    pub(super) fn item_of(
        &self,
        row: CompactTrack<String>,
    ) -> Result<QueueItem, DanglingReference> {
        Ok(QueueItem {
            uid: row.uid,
            track: QueueTrack {
                song_id: SongId::from(row.song_id),
                server_id: self.string(row.server_id)?,
                title: row.title.into_boxed_str(),
                artists: self.artist_list(row.artists)?,
                album: self.optional_string(row.album)?,
                album_id: self.optional_string(row.album_id)?,
                cover_art_id: self.optional_string(row.cover_art_id)?,
                duration_ms: row.duration_ms,
            },
        })
    }

    fn string(&self, index: usize) -> Result<Arc<str>, DanglingReference> {
        self.strings
            .get(index)
            .map(Arc::clone)
            .ok_or(DanglingReference)
    }

    fn optional_string(&self, index: Option<usize>) -> Result<Option<Arc<str>>, DanglingReference> {
        index.map(|index| self.string(index)).transpose()
    }

    fn artist_list(&self, index: usize) -> Result<Arc<[ArtistRef]>, DanglingReference> {
        self.artist_lists
            .get(index)
            .map(Arc::clone)
            .ok_or(DanglingReference)
    }
}
