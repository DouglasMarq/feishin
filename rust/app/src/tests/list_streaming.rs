use std::ops::Range;

use feishin_api::MusicServer;
use feishin_core::model::{Paging, SongListQuery, SongSort};
use feishin_core::queue::Uid;

use super::harness::{AudioCall, Harness, settle, start_harness_with, test_settings, test_songs};
use crate::controller::Command;
use crate::demo::test_song;
use crate::list_stream::{ListRow, Placement, PlaylistId, SongSource};

fn by_name() -> SongListQuery {
    SongListQuery {
        sort: SongSort::Name,
        ..SongListQuery::default()
    }
}

async fn listed_ids(harness: &Harness, query: &SongListQuery) -> Vec<String> {
    let paging = Paging {
        start_index: 0,
        limit: 100_000,
    };
    harness
        .server
        .song_list(query, paging)
        .await
        .map(|page| page.items.into_iter().map(|song| song.id).collect())
        .unwrap_or_default()
}

fn item_ids(harness: &Harness) -> Vec<String> {
    harness
        .controller
        .queue()
        .items()
        .iter()
        .map(|item| item.track.song_id.to_string())
        .collect()
}

fn sorted(mut ids: Vec<String>) -> Vec<String> {
    ids.sort();
    ids
}

fn single(range: Range<usize>) -> Vec<Range<usize>> {
    vec![range]
}

fn enqueue(harness: &mut Harness, ranges: Vec<Range<usize>>, placement: Placement) {
    harness.controller.handle(Command::EnqueueFromList {
        source: SongSource::Library(by_name()),
        ranges,
        placement,
    });
}

async fn streaming_harness(gapless: bool) -> (Harness, Vec<String>) {
    let harness = start_harness_with(test_settings(gapless, false), 1234, None);
    let expected = listed_ids(&harness, &by_name()).await;
    (harness, expected)
}

#[tokio::test]
async fn playback_starts_with_the_clicked_page_before_the_rest_streams_in() {
    let (mut harness, expected) = streaming_harness(false).await;
    let clicked = expected[700].clone();
    harness.request_from_list(by_name(), 700, &clicked);
    harness.handle_next_command().await;

    assert_eq!(harness.controller.queue().len(), 500);
    assert_eq!(harness.current_id(), Some(clicked));
    let uid = harness.current_uid().unwrap_or(Uid::MAX);
    assert_eq!(harness.audio.take(), [AudioCall::Load(uid, 0)]);

    harness.finish_streams().await;
    assert_eq!(harness.queued_ids(), expected);
    assert_eq!(harness.current_uid(), Some(uid));
    assert!(harness.audio.take().is_empty());
    assert_eq!(harness.ui.last_queue().map(|uids| uids.len()), Some(1234));
}

#[tokio::test]
async fn shuffled_streams_shuffle_the_whole_list_with_the_clicked_song_first() {
    let (mut harness, expected) = streaming_harness(false).await;
    harness.controller.handle(Command::ToggleShuffle);
    let clicked = expected[1100].clone();
    harness.play_from_list(by_name(), 1100, &clicked).await;

    let order = harness.queued_ids();
    assert_eq!(order[0], clicked);
    assert_ne!(order, expected);
    assert_eq!(sorted(order), sorted(expected.clone()));
    assert_eq!(item_ids(&harness), expected);
}

#[tokio::test]
async fn queue_edits_made_while_streaming_are_kept() {
    let (mut harness, expected) = streaming_harness(false).await;
    harness.request_from_list(by_name(), 700, &expected[700]);
    harness.handle_next_command().await;
    let stream_tail = harness.uid_of(&expected[999]).unwrap_or(Uid::MAX);
    harness
        .controller
        .handle(Command::Remove(vec![stream_tail]));
    harness
        .controller
        .handle(Command::AddNext(vec![test_song("next", 1000)]));
    harness
        .controller
        .handle(Command::AddLast(vec![test_song("last", 1000)]));
    harness.finish_streams().await;

    let mut wanted = expected.clone();
    wanted.remove(999);
    wanted.insert(701, "next".to_owned());
    wanted.push("last".to_owned());
    assert_eq!(harness.queued_ids(), wanted);
    assert_eq!(harness.current_id().as_ref(), Some(&expected[700]));
}

#[tokio::test]
async fn shuffled_streams_keep_songs_the_user_queued_in_place() {
    let (mut harness, expected) = streaming_harness(false).await;
    harness.controller.handle(Command::ToggleShuffle);
    harness.request_from_list(by_name(), 300, &expected[300]);
    harness.handle_next_command().await;
    harness
        .controller
        .handle(Command::AddNext(vec![test_song("next", 1000)]));
    harness
        .controller
        .handle(Command::AddLast(vec![test_song("last", 1000)]));
    harness.finish_streams().await;

    let order = harness.queued_ids();
    assert_eq!(order.len(), 1236);
    assert_eq!(order[0], expected[300]);
    assert_eq!(order[1], "next");
    assert_eq!(order.last().map(String::as_str), Some("last"));
}

#[tokio::test]
async fn a_newer_play_replaces_a_running_stream() {
    let (mut harness, expected) = streaming_harness(false).await;
    harness.request_from_list(by_name(), 700, &expected[700]);
    harness.handle_next_command().await;
    harness.request_from_list(by_name(), 100, &expected[100]);
    harness.finish_streams().await;

    assert_eq!(harness.current_id().as_ref(), Some(&expected[100]));
    assert_eq!(harness.queued_ids(), expected);
    assert_eq!(harness.audio.take().len(), 2);
}

#[tokio::test]
async fn playing_other_songs_stops_a_running_stream() {
    let (mut harness, expected) = streaming_harness(false).await;
    harness.request_from_list(by_name(), 700, &expected[700]);
    harness.handle_next_command().await;
    harness.controller.handle(Command::PlaySongs {
        songs: test_songs(3, 1000),
        song_id: None,
    });
    assert!(!harness.controller.is_streaming());
    settle().await;
    while let Ok(command) = harness.commands.try_recv() {
        harness.controller.handle(command);
    }
    assert_eq!(harness.queued_ids(), ["s0", "s1", "s2"]);
}

#[tokio::test]
async fn a_failing_page_keeps_playback_going_and_reports_the_incomplete_queue_once() {
    let (mut harness, expected) = streaming_harness(false).await;
    let playlist = harness
        .server
        .create_playlist("Big", &expected[..1200])
        .await;
    let playlist_id = playlist.map(|playlist| playlist.id).unwrap_or_default();
    harness.controller.handle(Command::PlayFromList {
        source: SongSource::Playlist(PlaylistId(playlist_id.clone())),
        song_id: None,
        row_index: ListRow(0),
    });
    harness.handle_next_command().await;
    assert!(harness.server.delete_playlist(&playlist_id).await.is_ok());
    harness.finish_streams().await;

    let queued = harness.controller.queue().len();
    assert!((500..1200).contains(&queued), "{queued}");
    let errors = harness.ui.errors();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].contains("incomplete"));
    assert!(harness.controller.is_playing());
    assert!(!harness.audio.take().contains(&AudioCall::Stop));
}

#[tokio::test]
async fn a_list_that_cannot_load_is_reported_and_the_queue_is_kept() {
    let (mut harness, _) = streaming_harness(false).await;
    harness.controller.play_now(test_songs(2, 1000), "s0");
    harness.controller.handle(Command::PlayFromList {
        source: SongSource::Playlist(PlaylistId("missing".into())),
        song_id: None,
        row_index: ListRow(0),
    });
    harness.finish_streams().await;

    let errors = harness.ui.errors();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].starts_with("Could not load the list"));
    assert_eq!(harness.queued_ids(), ["s0", "s1"]);
}

#[tokio::test]
async fn an_empty_list_has_nothing_to_play() {
    let (mut harness, _) = streaming_harness(false).await;
    let query = SongListQuery {
        search: Some("no song is called like this".into()),
        ..by_name()
    };
    harness.controller.handle(Command::PlayFromList {
        source: SongSource::Library(query),
        song_id: None,
        row_index: ListRow(0),
    });
    harness.finish_streams().await;
    assert_eq!(
        harness.ui.errors(),
        ["There is nothing to play in this list"]
    );
    assert!(harness.audio.take().is_empty());
}

#[tokio::test]
async fn enqueued_ranges_stream_next_and_last_in_list_order() {
    let (mut harness, expected) = streaming_harness(false).await;
    harness.controller.play_now(test_songs(2, 1000), "s0");
    harness.audio.take();
    enqueue(&mut harness, vec![600..1100, 10..20], Placement::Next);
    harness.finish_streams().await;
    enqueue(&mut harness, single(0..2), Placement::Last);
    harness.finish_streams().await;

    let wanted = [
        vec!["s0".to_owned()],
        expected[10..20].to_vec(),
        expected[600..1100].to_vec(),
        vec!["s1".to_owned()],
        expected[0..2].to_vec(),
    ]
    .concat();
    assert_eq!(harness.queued_ids(), wanted);
    assert_eq!(harness.current_id().as_deref(), Some("s0"));
    assert!(harness.audio.take().is_empty());
}

#[tokio::test]
async fn enqueueing_now_while_shuffled_starts_at_a_random_selected_song() {
    let (mut harness, expected) = streaming_harness(false).await;
    harness.controller.handle(Command::ToggleShuffle);
    enqueue(&mut harness, single(100..900), Placement::Now);
    harness.finish_streams().await;

    let order = harness.queued_ids();
    assert_eq!(sorted(order.clone()), sorted(expected[100..900].to_vec()));
    assert_eq!(harness.current_id().as_ref(), order.first());
    assert_eq!(harness.audio.take().len(), 1);
}

#[tokio::test]
async fn streamed_pages_refresh_the_gapless_preload() {
    let (mut harness, expected) = streaming_harness(true).await;
    harness.request_from_list(by_name(), 999, &expected[999]);
    harness.handle_next_command().await;
    harness.start_current();
    harness.audio.take();
    harness.finish_streams().await;

    let following = harness.uid_of(&expected[1000]).unwrap_or(Uid::MAX);
    assert_eq!(harness.audio.take(), [AudioCall::Preload(following)]);
}
