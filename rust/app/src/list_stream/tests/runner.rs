use super::support::{FakePages, Recorded, ids, run_to_end, spawn_runner};
use crate::list_stream::message::StreamEvent;
use crate::list_stream::rows::{ListRow, RowRanges};
use crate::list_stream::start::StreamStart;

fn at_row(row: usize, song_id: Option<&str>) -> StreamStart {
    StreamStart::Row {
        row: ListRow(row),
        song_id: song_id.map(str::to_owned),
    }
}

#[tokio::test]
async fn the_clicked_page_comes_first_then_the_pages_after_and_before_it() {
    let recorded = run_to_end(
        FakePages::new(1234),
        RowRanges::whole_list(),
        at_row(700, Some("song-700")),
    )
    .await;
    assert_eq!(
        recorded,
        [
            Recorded::Start(ids(500, 1000), 200),
            Recorded::After(ids(1000, 1234)),
            Recorded::Before(ids(0, 500)),
            Recorded::Finished,
        ]
    );
}

#[tokio::test]
async fn the_start_follows_the_song_and_falls_back_to_the_page_top() {
    let moved = run_to_end(
        FakePages::new(600),
        RowRanges::whole_list(),
        at_row(3, Some("song-7")),
    )
    .await;
    assert_eq!(moved.first(), Some(&Recorded::Start(ids(0, 500), 7)));
    let missing = run_to_end(
        FakePages::new(600),
        RowRanges::whole_list(),
        at_row(3, Some("gone")),
    )
    .await;
    assert_eq!(missing.first(), Some(&Recorded::Start(ids(0, 500), 0)));
    let unloaded = run_to_end(
        FakePages::new(600),
        RowRanges::whole_list(),
        at_row(503, None),
    )
    .await;
    assert_eq!(unloaded.first(), Some(&Recorded::Start(ids(500, 600), 3)));
}

#[tokio::test]
async fn a_row_past_the_end_starts_at_the_top_of_the_list() {
    let recorded = run_to_end(
        FakePages::new(700),
        RowRanges::whole_list(),
        at_row(5000, None),
    )
    .await;
    assert_eq!(
        recorded,
        [
            Recorded::Start(ids(0, 500), 0),
            Recorded::After(ids(500, 700)),
            Recorded::Finished,
        ]
    );
}

#[tokio::test]
async fn ranges_only_fetch_and_keep_their_rows_in_list_order() {
    let pages = FakePages::new(5000);
    let listed = RowRanges::from_ranges(vec![600..1100, 10..20, 3..5]);
    let recorded = run_to_end(pages.clone(), listed, StreamStart::FirstRow).await;
    let first_page = [ids(3, 5), ids(10, 20)].concat();
    assert_eq!(
        recorded,
        [
            Recorded::Start(first_page, 0),
            Recorded::After(ids(600, 1000)),
            Recorded::After(ids(1000, 1100)),
            Recorded::Finished,
        ]
    );
    assert_eq!(pages.requested(), [0, 500, 1000]);
}

#[tokio::test]
async fn without_a_total_a_short_or_empty_page_ends_the_list() {
    let mut short = FakePages::new(1100);
    short.reports_total = false;
    let recorded = run_to_end(
        short.clone(),
        RowRanges::whole_list(),
        StreamStart::FirstRow,
    )
    .await;
    assert_eq!(recorded.len(), 4);
    assert_eq!(short.requested(), [0, 500, 1000]);
    let mut exact = FakePages::new(1000);
    exact.reports_total = false;
    let recorded = run_to_end(
        exact.clone(),
        RowRanges::whole_list(),
        StreamStart::FirstRow,
    )
    .await;
    assert_eq!(recorded.last(), Some(&Recorded::Finished));
    assert_eq!(exact.requested(), [0, 500, 1000]);
}

#[tokio::test]
async fn flaky_pages_are_retried() {
    let pages = FakePages::new(1200).failing_at(500, Some(2));
    let recorded = run_to_end(
        pages.clone(),
        RowRanges::whole_list(),
        StreamStart::FirstRow,
    )
    .await;
    assert_eq!(recorded.len(), 4);
    assert_eq!(recorded.last(), Some(&Recorded::Finished));
    assert_eq!(pages.requested(), [0, 500, 500, 500, 1000]);
}

#[tokio::test]
async fn a_page_that_keeps_failing_stops_the_stream_once() {
    let pages = FakePages::new(1200).failing_at(500, None);
    let recorded = run_to_end(
        pages.clone(),
        RowRanges::whole_list(),
        StreamStart::FirstRow,
    )
    .await;
    assert_eq!(
        recorded,
        [Recorded::Start(ids(0, 500), 0), Recorded::Failed]
    );
    assert_eq!(pages.requested(), [0, 500, 500, 500]);
}

#[tokio::test]
async fn a_random_start_stays_inside_the_listed_rows() {
    for _ in 0..20 {
        let listed = RowRanges::from_ranges(vec![600..610, 1990..2000]);
        let recorded = run_to_end(FakePages::new(1995), listed, StreamStart::RandomRow).await;
        let Some(Recorded::Start(songs, start_index)) = recorded.first() else {
            panic!("expected a start page, got {recorded:?}");
        };
        let started = &songs[*start_index];
        let in_first = ids(600, 610).contains(started);
        let in_second = ids(1990, 1995).contains(started);
        assert!(in_first || in_second, "{started}");
    }
}

#[tokio::test]
async fn dropping_a_page_unapplied_cancels_the_stream() {
    let pages = FakePages::new(5000);
    let mut messages = spawn_runner(
        pages.clone(),
        RowRanges::whole_list(),
        StreamStart::FirstRow,
    );
    let first = messages.recv().await.map(|message| message.event);
    assert!(matches!(first, Some(StreamEvent::Start { .. })));
    drop(first);
    assert!(messages.recv().await.is_none());
    assert_eq!(pages.requested(), [0, 500]);
}
