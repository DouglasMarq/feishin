use feishin_core::model::{Paging, SongListQuery};
use slint::Model;

use super::super::model::{PAGE_SIZE, TracksModel, paging_for_page};
use super::{RequestLog, drain, logging_model, page_of, songs_in};
use crate::demo::FakeServer;

const CACHED_PAGES: usize = 24;
const PAGES: usize = 40;
const TOTAL: usize = PAGES * PAGE_SIZE;

fn requested_pages(log: &RequestLog) -> Vec<usize> {
    drain(log)
        .into_iter()
        .filter_map(|(_, page)| page)
        .collect()
}

fn scroll_to(model: &TracksModel, log: &RequestLog, row: usize) {
    model.row_data(row);
    for page in requested_pages(log) {
        model.apply_page(1, page, page_of(page, PAGE_SIZE));
    }
}

fn counted_model() -> (TracksModel, RequestLog) {
    let (model, log) = logging_model();
    model.reset(SongListQuery::default());
    assert!(model.apply_count(1, TOTAL));
    drain(&log);
    (model, log)
}

fn loaded_page_count(model: &TracksModel) -> usize {
    (0..PAGES)
        .filter(|page| model.song_id_at(page * PAGE_SIZE).is_some())
        .count()
}

#[test]
fn a_full_scroll_keeps_only_the_bounded_window_of_pages() {
    let (model, log) = counted_model();
    for row in (0..TOTAL).step_by(PAGE_SIZE / 4) {
        scroll_to(&model, &log, row);
        assert!(loaded_page_count(&model) <= CACHED_PAGES);
    }
    assert_eq!(loaded_page_count(&model), CACHED_PAGES);
    assert_eq!(model.song_id_at(TOTAL - 1).as_deref(), Some("s3999"));
    assert_eq!(model.song_id_at(0), None);
    assert_eq!(
        model.song_id_at((PAGES - CACHED_PAGES) * PAGE_SIZE - 1),
        None
    );
    assert!(
        model
            .song_id_at((PAGES - CACHED_PAGES) * PAGE_SIZE)
            .is_some()
    );
}

#[test]
fn scrolling_back_refetches_evicted_pages_with_placeholders_meanwhile() {
    let (model, log) = counted_model();
    for row in (0..TOTAL).step_by(PAGE_SIZE) {
        scroll_to(&model, &log, row);
    }
    model.set_current(Some("s5"));
    assert!(model.row_data(5).is_some_and(|row| !row.loaded));
    assert_eq!(requested_pages(&log), [0]);
    assert!(model.row_data(6).is_some_and(|row| !row.loaded));
    assert!(requested_pages(&log).is_empty());

    model.apply_page(1, 0, page_of(0, PAGE_SIZE));
    assert!(
        model
            .row_data(5)
            .is_some_and(|row| row.loaded && row.playing)
    );
    model.row_data(150);
    assert_eq!(requested_pages(&log), [1, 2]);
}

#[test]
fn stale_generations_never_fill_the_new_cache() {
    let (model, log) = counted_model();
    model.row_data(TOTAL - 1);
    assert_eq!(requested_pages(&log), [PAGES - 1]);
    model.reset(SongListQuery::default());
    assert!(model.apply_count(2, TOTAL));
    drain(&log);

    model.apply_page(1, PAGES - 1, page_of(PAGES - 1, PAGE_SIZE));
    model.page_failed(1, 0);
    assert_eq!(model.song_id_at(TOTAL - 1), None);
    model.row_data(TOTAL - 1);
    assert_eq!(drain(&log), [(2, Some(PAGES - 1))]);
}

#[test]
fn edits_and_highlights_skip_evicted_rows() {
    let (model, log) = counted_model();
    for row in (0..TOTAL).step_by(PAGE_SIZE) {
        scroll_to(&model, &log, row);
    }
    model.update_songs(&["s5".into(), "s3990".into()], |song| song.favorite = true);
    model.set_current(Some("s7"));
    assert!(model.row_data(3990).is_some_and(|row| row.favorite));
    scroll_to(&model, &log, 0);
    assert!(
        model
            .row_data(7)
            .is_some_and(|row| row.playing && !row.favorite)
    );
}

async fn load_from_server(model: &TracksModel, log: &RequestLog, server: &FakeServer, row: usize) {
    model.row_data(row);
    let query = model.query();
    for page in requested_pages(log) {
        let songs = songs_in(server, &query, paging_for_page(page)).await;
        model.apply_page(1, page, songs);
    }
}

#[tokio::test]
async fn selections_spanning_evicted_pages_resolve_through_the_server() {
    let server = FakeServer::new(TOTAL);
    let (model, log) = counted_model();
    load_from_server(&model, &log, &server, 0).await;
    model.select(5, false, false);
    for row in (0..TOTAL).step_by(PAGE_SIZE) {
        load_from_server(&model, &log, &server, row).await;
    }
    assert_eq!(model.select(TOTAL - 5, false, true), TOTAL - 9);
    assert!(
        model
            .row_data(5)
            .is_some_and(|row| row.selected && !row.loaded)
    );

    let snapshot = model.selection();
    assert!(snapshot.missing_pages().contains(&0));
    let first = model.selection().first_only();
    assert!(!first.is_loaded());

    let selected_rows = Paging {
        start_index: 5,
        limit: TOTAL - 9,
    };
    let expected = songs_in(&server, &model.query(), selected_rows).await;
    let resolved = snapshot.resolve(&server).await.unwrap_or_default();
    assert_eq!(resolved, expected);
    let first_song = first.resolve(&server).await.unwrap_or_default();
    assert_eq!(first_song.first(), expected.first());
}
