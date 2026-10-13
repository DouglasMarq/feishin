use feishin_core::model::ArtistRef;
use feishin_core::queue::{QueueItem, QueueSnapshotRef};
use serde::ser::{Error as _, SerializeSeq};
use serde::{Serialize, Serializer};

use super::compact::{CompactQueue, CompactTrack, QUEUE_FORMAT};
use super::shared_table::{MissingSharedValue, SharedTable};

#[derive(Default)]
struct SharedTables<'queue> {
    strings: SharedTable<'queue, str>,
    artist_lists: SharedTable<'queue, [ArtistRef]>,
}

struct TrackRows<'tables, 'queue> {
    items: &'queue [QueueItem],
    tables: &'tables SharedTables<'queue>,
}

pub(super) fn encode(view: &QueueSnapshotRef<'_>, position_ms: u64) -> crate::Result<Vec<u8>> {
    let tables = SharedTables::of(view.items);
    let compact = CompactQueue {
        strings: tables.strings.values(),
        artist_lists: tables.artist_lists.values(),
        tracks: TrackRows {
            items: view.items,
            tables: &tables,
        },
        shuffled: view.shuffled,
        current_uid: view.current_uid,
        repeat: view.repeat,
        next_uid: view.next_uid,
        position_ms,
    };
    QUEUE_FORMAT.encode(&compact)
}

impl<'queue> SharedTables<'queue> {
    fn of(items: &'queue [QueueItem]) -> Self {
        let mut tables = Self::default();
        for item in items {
            tables.add_track(item);
        }
        tables
    }

    fn add_track(&mut self, item: &'queue QueueItem) {
        let track = &item.track;
        self.artist_lists.add(&track.artists);
        let shared_strings = [
            Some(&track.server_id),
            track.album.as_ref(),
            track.album_id.as_ref(),
            track.cover_art_id.as_ref(),
        ];
        for value in shared_strings.into_iter().flatten() {
            self.strings.add(value);
        }
    }

    fn row_of(
        &self,
        item: &'queue QueueItem,
    ) -> Result<CompactTrack<&'queue str>, MissingSharedValue> {
        let track = &item.track;
        Ok(CompactTrack {
            uid: item.uid,
            song_id: track.song_id.as_str(),
            title: &track.title,
            server_id: self.strings.index_of(&track.server_id)?,
            artists: self.artist_lists.index_of(&track.artists)?,
            album: self.strings.optional_index_of(track.album.as_ref())?,
            album_id: self.strings.optional_index_of(track.album_id.as_ref())?,
            cover_art_id: self
                .strings
                .optional_index_of(track.cover_art_id.as_ref())?,
            duration_ms: track.duration_ms,
        })
    }
}

impl Serialize for TrackRows<'_, '_> {
    fn serialize<RowSerializer: Serializer>(
        &self,
        serializer: RowSerializer,
    ) -> Result<RowSerializer::Ok, RowSerializer::Error> {
        let mut rows = serializer.serialize_seq(Some(self.items.len()))?;
        for item in self.items {
            let row = self
                .tables
                .row_of(item)
                .map_err(RowSerializer::Error::custom)?;
            rows.serialize_element(&row)?;
        }
        rows.end()
    }
}
