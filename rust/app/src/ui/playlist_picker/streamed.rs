use std::collections::HashSet;

use feishin_api::{ApiError, MusicServer};

use crate::list_stream::{PlaylistId, RowRanges, SongSource};
use crate::ui::tracks::{LISTED_CHUNK_SIZE, ListedChunks, ListedSongs, SelectionRanges};

#[derive(Debug, Default)]
pub struct ListedAdd {
    pub added: usize,
    pub read: usize,
    pub failure: Option<ApiError>,
}

impl ListedAdd {
    pub fn failed(error: ApiError) -> Self {
        Self {
            failure: Some(error),
            ..Self::default()
        }
    }
}

struct ChunkAdded {
    read: usize,
    added: usize,
}

pub async fn existing_song_ids(
    server: &dyn MusicServer,
    playlist_id: &str,
) -> feishin_api::Result<HashSet<String>> {
    let whole_playlist = ListedSongs {
        source: SongSource::Playlist(PlaylistId(playlist_id.to_owned())),
        ranges: SelectionRanges::spanning(RowRanges::WHOLE_LIST),
    };
    let mut chunks = whole_playlist.into_chunks(LISTED_CHUNK_SIZE);
    let mut existing = HashSet::new();
    while let Some(song_ids) = chunks.next_ids(server).await? {
        existing.extend(song_ids);
    }
    Ok(existing)
}

pub async fn add_listed_songs(
    server: &dyn MusicServer,
    playlist_id: &str,
    listed: ListedSongs,
    existing: &HashSet<String>,
    report: impl Fn(usize),
) -> ListedAdd {
    let mut chunks = listed.into_chunks(LISTED_CHUNK_SIZE);
    let mut outcome = ListedAdd::default();
    loop {
        match add_next_chunk(server, playlist_id, &mut chunks, existing).await {
            Ok(Some(chunk)) => {
                outcome.read += chunk.read;
                outcome.added += chunk.added;
                report(outcome.read);
            }
            Ok(None) => return outcome,
            Err(error) => {
                outcome.failure = Some(error);
                return outcome;
            }
        }
    }
}

async fn add_next_chunk(
    server: &dyn MusicServer,
    playlist_id: &str,
    chunks: &mut ListedChunks,
    existing: &HashSet<String>,
) -> feishin_api::Result<Option<ChunkAdded>> {
    let Some(song_ids) = chunks.next_ids(server).await? else {
        return Ok(None);
    };
    let read = song_ids.len();
    let new_ids = unseen_ids(existing, song_ids);
    if !new_ids.is_empty() {
        server.add_to_playlist(playlist_id, &new_ids).await?;
    }
    Ok(Some(ChunkAdded {
        read,
        added: new_ids.len(),
    }))
}

fn unseen_ids(existing: &HashSet<String>, song_ids: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    song_ids
        .into_iter()
        .filter(|song_id| !existing.contains(song_id) && seen.insert(song_id.clone()))
        .collect()
}
