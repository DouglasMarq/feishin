use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use feishin_api::MusicServer;
use feishin_core::model::{Page, Song};
use slint::Model;

use super::model::{PageRequest, PlaylistSongsModel};
use super::pages::{MAX_CACHED_PAGES, PAGE_SIZE, paging_for_page};
use super::selection::ClickMode;
use crate::demo::{FakeServer, test_song};
use crate::ui::page_cache::PageApplied;

type RequestLog = Rc<RefCell<Vec<PageRequest>>>;

fn logging_model() -> (PlaylistSongsModel, RequestLog) {
    let log: RequestLog = Rc::default();
    let sink = Rc::clone(&log);
    let model = PlaylistSongsModel::new(move |request| sink.borrow_mut().push(request));
    (model, log)
}

fn requested_pages(log: &RequestLog) -> Vec<(u64, usize)> {
    log.borrow_mut()
        .drain(..)
        .map(|request| (request.generation, request.page))
        .collect()
}

fn page_of_songs(page: usize, len: usize) -> Vec<Song> {
    (0..len)
        .map(|position| test_song(&format!("s{}", page * PAGE_SIZE + position), 1000))
        .collect()
}

#[test]
fn rows_load_on_demand_page_by_page() {
    let (model, log) = logging_model();
    model.reset(1, "playlist-1");
    assert_eq!(model.row_count(), 0);
    assert!(model.apply_first_page(1, 250, page_of_songs(0, PAGE_SIZE)));
    assert_eq!(model.row_count(), 250);
    assert!(model.row_data(5).is_some_and(|row| row.track.loaded));
    assert!(requested_pages(&log).is_empty());

    let placeholder = model.row_data(130);
    assert!(placeholder.is_some_and(|row| !row.track.loaded && row.track.index == 131));
    model.row_data(140);
    assert_eq!(requested_pages(&log), [(1, 1)]);
    assert!(model.row_data(250).is_none());

    model.apply_page(1, 1, page_of_songs(1, PAGE_SIZE));
    assert_eq!(model.song_id_at(130).as_deref(), Some("s130"));
}

#[test]
fn stale_generations_are_ignored_and_failures_back_off() {
    let (model, log) = logging_model();
    model.reset(1, "playlist-1");
    model.reset(2, "playlist-2");
    assert!(!model.apply_first_page(1, 50, page_of_songs(0, 50)));
    assert_eq!(model.row_count(), 0);
    assert!(model.apply_first_page(2, 300, page_of_songs(0, PAGE_SIZE)));
    model.row_data(220);
    assert_eq!(requested_pages(&log), [(2, 2)]);
    assert!(!model.page_failed(&page_request(1, 0, 2)));
    model.row_data(220);
    assert!(requested_pages(&log).is_empty());
    assert!(model.page_failed(&page_request(2, 0, 2)));
    model.row_data(220);
    assert!(requested_pages(&log).is_empty());
}

fn page_request(generation: u64, revision: u64, page: usize) -> PageRequest {
    PageRequest {
        generation,
        revision,
        playlist_id: "playlist-1".into(),
        page,
    }
}

fn take_requests(log: &RequestLog) -> Vec<PageRequest> {
    log.borrow_mut().drain(..).collect()
}

fn listed(page: usize, total: usize) -> Page<Song> {
    Page {
        items: page_of_songs(page, PAGE_SIZE),
        start_index: page * PAGE_SIZE,
        total_count: Some(total),
    }
}

fn grown_playlist_model() -> (PlaylistSongsModel, RequestLog) {
    let (model, log) = logging_model();
    model.reset(1, "playlist-1");
    model.apply_first_page(1, 10 * PAGE_SIZE, page_of_songs(0, PAGE_SIZE));
    model.row_data(4 * PAGE_SIZE);
    let requests = take_requests(&log);
    let [fourth] = requests.as_slice() else {
        panic!("expected one page request, got {}", requests.len());
    };
    let grown = model.apply_requested_page(fourth, listed(4, 10 * PAGE_SIZE + 1));
    assert_eq!(grown, PageApplied::TotalChanged(10 * PAGE_SIZE + 1));
    (model, log)
}

#[test]
fn a_page_that_failed_before_the_playlist_changed_does_not_hold_back_the_new_rows() {
    let (model, log) = grown_playlist_model();
    model.row_data(5);
    assert_eq!(requested_pages(&log), [(1, 0)]);
    assert!(!model.page_failed(&page_request(1, 0, 0)));
    model.apply_requested_page(&page_request(1, 1, 0), listed(0, 10 * PAGE_SIZE + 1));
    assert_eq!(model.song_id_at(5).as_deref(), Some("s5"));
}

#[test]
fn a_playlist_that_changed_size_drops_the_selection() {
    let (model, _log) = logging_model();
    model.reset(1, "playlist-1");
    model.apply_first_page(1, 10 * PAGE_SIZE, page_of_songs(0, PAGE_SIZE));
    model.click(3, ClickMode::Select);
    assert_eq!(model.selected_indexes(), [3]);
    let changed = model.apply_requested_page(&page_request(1, 0, 4), listed(4, 10 * PAGE_SIZE - 1));
    assert_eq!(changed, PageApplied::TotalChanged(10 * PAGE_SIZE - 1));
    assert!(model.selected_indexes().is_empty());
}

#[test]
fn a_playlist_that_changed_size_on_the_server_reloads_its_pages() {
    let (model, log) = logging_model();
    model.reset(1, "playlist-1");
    model.apply_first_page(1, 10 * PAGE_SIZE, page_of_songs(0, PAGE_SIZE));
    model.row_data(3 * PAGE_SIZE);
    model.row_data(4 * PAGE_SIZE);
    let requests = take_requests(&log);
    let [third, fourth] = requests.as_slice() else {
        panic!("expected two page requests, got {}", requests.len());
    };
    let unchanged = model.apply_requested_page(third, listed(3, 10 * PAGE_SIZE));
    assert_eq!(unchanged, PageApplied::Applied);
    let shrunk = model.apply_requested_page(fourth, listed(4, 10 * PAGE_SIZE - 1));
    assert_eq!(shrunk, PageApplied::TotalChanged(10 * PAGE_SIZE - 1));
    assert_eq!(model.row_count(), 10 * PAGE_SIZE - 1);
    assert!(model.song_id_at(0).is_none());
    assert!(model.song_id_at(3 * PAGE_SIZE).is_none());
    assert_eq!(model.song_id_at(4 * PAGE_SIZE).as_deref(), Some("s400"));
    assert_eq!(
        model.apply_requested_page(third, listed(3, 10 * PAGE_SIZE)),
        PageApplied::Stale
    );
    model.row_data(5);
    let refetched = take_requests(&log);
    assert_eq!(
        refetched
            .iter()
            .map(|request| (request.revision, request.page))
            .collect::<Vec<_>>(),
        [(1, 0)]
    );
}

#[test]
fn selection_marks_rows_and_counts() {
    let (model, _log) = logging_model();
    model.reset(1, "playlist-1");
    model.apply_first_page(1, 120, page_of_songs(0, PAGE_SIZE));
    assert_eq!(model.click(2, ClickMode::Select), 1);
    assert_eq!(model.click(5, ClickMode::SelectRange), 4);
    assert!(model.row_data(3).is_some_and(|row| row.selected));
    assert_eq!(model.click(500, ClickMode::Toggle), 4);
    assert_eq!(model.targets(4), [2, 3, 4, 5]);
    assert_eq!(model.targets(9), [9]);
    assert_eq!(model.select_all(), 120);
    assert!(model.row_data(110).is_some_and(|row| row.selected));
    assert_eq!(model.clear_selection(), 0);
    assert!(model.selected_indexes().is_empty());
}

#[tokio::test]
async fn snapshots_resolve_rows_that_are_not_cached() {
    let server = FakeServer::new(2000);
    let song_ids: Vec<String> = (0..450).map(|index| format!("song-{index}")).collect();
    let created = server.create_playlist("Big", &song_ids).await.unwrap();
    let (model, _log) = logging_model();
    model.reset(1, &created.id);
    let first = server
        .playlist_song_page(&created.id, paging_for_page(0))
        .await
        .unwrap();
    model.apply_first_page(1, first.total_count.unwrap_or_default(), first.items);

    let loaded = model.snapshot(&[1, 2]);
    assert!(loaded.is_loaded());
    let mixed = model.snapshot(&[3, 320, 449, 9999]);
    assert!(!mixed.is_loaded());
    assert_eq!(
        mixed.missing_pages().into_iter().collect::<Vec<_>>(),
        [3, 4]
    );
    let songs = mixed.resolve(&server).await.unwrap();
    let ids: Vec<&str> = songs.iter().map(|song| song.id.as_str()).collect();
    assert_eq!(ids, ["song-3", "song-320", "song-449"]);
}

#[tokio::test]
async fn scrolling_a_large_playlist_keeps_a_bounded_cache() {
    let server = FakeServer::new(20_000);
    let song_ids: Vec<String> = (0..10_000).map(|index| format!("song-{index}")).collect();
    let created = server.create_playlist("Huge", &song_ids).await.unwrap();
    let (model, log) = logging_model();
    model.reset(1, &created.id);
    let first = server
        .playlist_song_page(&created.id, paging_for_page(0))
        .await
        .unwrap();
    model.apply_first_page(1, first.total_count.unwrap_or_default(), first.items);
    assert_eq!(model.row_count(), 10_000);

    let started = Instant::now();
    let mut fetched_pages = 0;
    for row in (0..model.row_count()).step_by(20) {
        model.row_data(row);
        let requests: Vec<PageRequest> = log.borrow_mut().drain(..).collect();
        for request in requests {
            let page = server
                .playlist_song_page(&request.playlist_id, paging_for_page(request.page))
                .await
                .unwrap();
            model.apply_page(request.generation, request.page, page.items);
            fetched_pages += 1;
        }
    }
    let cached_songs = (0..model.row_count())
        .filter(|row| model.song_id_at(*row).is_some())
        .count();
    println!(
        "scrolled 10000 playlist rows in {:?}: {fetched_pages} page loads, {cached_songs} songs cached",
        started.elapsed()
    );
    assert_eq!(fetched_pages, 10_000 / PAGE_SIZE - 1);
    assert!(cached_songs <= MAX_CACHED_PAGES * PAGE_SIZE);
    assert_eq!(model.song_id_at(9_999).as_deref(), Some("song-9999"));
}

#[test]
fn late_pages_far_from_the_view_are_dropped_and_requested_again() {
    let (model, log) = logging_model();
    model.reset(1, "playlist-1");
    model.apply_first_page(1, 20 * PAGE_SIZE, page_of_songs(0, PAGE_SIZE));
    model.row_data(19 * PAGE_SIZE + 50);
    assert_eq!(requested_pages(&log), [(1, 19)]);
    for page in 1..MAX_CACHED_PAGES {
        model.row_data(page * PAGE_SIZE);
        model.apply_page(1, page, page_of_songs(page, PAGE_SIZE));
    }
    assert_eq!(requested_pages(&log).len(), MAX_CACHED_PAGES - 1);

    model.apply_page(1, 19, page_of_songs(19, PAGE_SIZE));
    assert!(model.song_id_at(19 * PAGE_SIZE + 50).is_none());
    assert!(model.song_id_at(0).is_some());
    model.row_data(19 * PAGE_SIZE + 50);
    assert_eq!(requested_pages(&log), [(1, 19)]);
}
