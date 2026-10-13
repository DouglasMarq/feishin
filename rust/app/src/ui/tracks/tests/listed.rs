use feishin_core::model::{Paging, Song, SongListQuery};

use super::super::listed::ListedSongs;
use super::super::selection_ranges::SelectionRanges;
use super::{listed_songs, songs_in};
use crate::demo::FakeServer;
use crate::list_stream::SongSource;

fn library_rows(ranges: Vec<std::ops::Range<usize>>) -> ListedSongs {
    ListedSongs {
        source: SongSource::Library(SongListQuery::default()),
        ranges: SelectionRanges::from_ranges(ranges),
    }
}

async fn rows_in(server: &FakeServer, start_index: usize, limit: usize) -> Vec<Song> {
    let paging = Paging { start_index, limit };
    songs_in(server, &SongListQuery::default(), paging).await
}

#[tokio::test]
async fn listed_rows_stream_in_bounded_chunks_in_list_order() {
    let server = FakeServer::new(2000);
    let listed = library_rows(vec![1200..1800, 5..7, 230..232]);
    assert_eq!(listed.song_count(), 604);

    let mut chunks = listed.clone().into_chunks(500);
    let mut chunk_sizes = Vec::new();
    while let Ok(Some(chunk)) = chunks.next_songs(&server).await {
        chunk_sizes.push(chunk.len());
    }
    assert_eq!(chunk_sizes, [2, 2, 500, 100]);

    let mut expected = rows_in(&server, 5, 2).await;
    expected.extend(rows_in(&server, 230, 2).await);
    expected.extend(rows_in(&server, 1200, 600).await);
    assert_eq!(listed_songs(&server, listed).await, expected);
}

#[tokio::test]
async fn listed_rows_stop_where_the_list_ends() {
    let server = FakeServer::new(500);
    let listed = library_rows(vec![450..600, 700..800]);
    let mut chunks = listed.into_chunks(40);
    let mut ids = Vec::new();
    while let Ok(Some(chunk_ids)) = chunks.next_ids(&server).await {
        ids.extend(chunk_ids);
    }
    let expected: Vec<String> = rows_in(&server, 450, 50)
        .await
        .into_iter()
        .map(|song| song.id)
        .collect();
    assert_eq!(ids, expected);
}
