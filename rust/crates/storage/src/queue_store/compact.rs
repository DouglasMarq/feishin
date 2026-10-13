use feishin_core::queue::{RepeatMode, Uid};
use serde::{Deserialize, Serialize};

use super::QUEUE_FILE;
use super::codec::FileFormat;

pub(super) const QUEUE_FORMAT: FileFormat = FileFormat {
    file_name: QUEUE_FILE,
    magic: *b"FSQ1",
};

#[derive(Serialize)]
pub(super) struct CompactQueue<Strings, ArtistLists, Tracks, PlayOrder> {
    pub(super) strings: Strings,
    pub(super) artist_lists: ArtistLists,
    pub(super) tracks: Tracks,
    pub(super) shuffled: Option<PlayOrder>,
    pub(super) current_uid: Option<Uid>,
    pub(super) repeat: RepeatMode,
    pub(super) next_uid: Uid,
    pub(super) position_ms: u64,
}

#[derive(Serialize, Deserialize)]
pub(super) struct CompactTrack<Text> {
    pub(super) uid: Uid,
    pub(super) song_id: Text,
    pub(super) title: Text,
    pub(super) server_id: usize,
    pub(super) artists: usize,
    pub(super) album: Option<usize>,
    pub(super) album_id: Option<usize>,
    pub(super) cover_art_id: Option<usize>,
    pub(super) duration_ms: u64,
}
