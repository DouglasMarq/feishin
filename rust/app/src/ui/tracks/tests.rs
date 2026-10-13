mod eviction;
mod headless;
mod listed;
mod ranges;

use std::cell::RefCell;
use std::rc::Rc;

use feishin_api::MusicServer;
use feishin_core::model::{Paging, Song, SongListQuery, SongSort};
use slint::Model;

use super::listed::{LISTED_CHUNK_SIZE, ListedSongs};
use super::model::{PAGE_SIZE, Request, TracksModel};
use super::query::{Filters, parse_year, sort_for};
use super::row::track_row;
use super::selection::{SelectedSongs, Selection};
use super::selection_ranges::SelectionRanges;
use crate::demo::{FakeServer, test_song};

type RequestLog = Rc<RefCell<Vec<(u64, Option<usize>)>>>;

fn drain(log: &RequestLog) -> Vec<(u64, Option<usize>)> {
    log.borrow_mut().drain(..).collect()
}

fn logging_model() -> (TracksModel, RequestLog) {
    let log: RequestLog = Rc::default();
    let sink = Rc::clone(&log);
    let model = TracksModel::new(move |request| {
        sink.borrow_mut().push(match request {
            Request::Count { generation, .. } => (generation, None),
            Request::Page {
                generation, page, ..
            } => (generation, Some(page)),
        });
    });
    (model, log)
}

fn page_of(page: usize, len: usize) -> Vec<Song> {
    (0..len)
        .map(|position| test_song(&format!("s{}", page * PAGE_SIZE + position), 1000))
        .collect()
}

#[test]
fn pages_are_requested_once_and_stale_generations_ignored() {
    let (model, log) = logging_model();
    model.reset(SongListQuery::default());
    assert_eq!(drain(&log), [(1, None)]);
    assert!(model.apply_count(1, 250));
    assert_eq!(model.row_count(), 250);

    assert!(model.row_data(150).is_some_and(|row| !row.loaded));
    model.row_data(160);
    assert_eq!(drain(&log), [(1, Some(1)), (1, Some(2))]);

    model.apply_page(1, 1, vec![test_song("x", 1000)]);
    model.set_current(Some("x"));
    assert!(
        model
            .row_data(100)
            .is_some_and(|row| row.loaded && row.playing)
    );
    assert_eq!(model.song_id_at(100).as_deref(), Some("x"));

    model.reset(SongListQuery::default());
    assert!(!model.apply_count(1, 10));
    assert_eq!(model.row_count(), 0);
}

fn rows_of(ranges: &SelectionRanges) -> Vec<usize> {
    ranges.rows().collect()
}

#[test]
fn clicks_follow_desktop_selection_rules() {
    let mut selection = Selection::default();
    assert_eq!(rows_of(&selection.click(3, false, false)), [3]);
    assert_eq!(rows_of(&selection.click(5, false, true)), [4, 5]);
    assert_eq!(rows_of(selection.ranges()), [3, 4, 5]);

    assert_eq!(rows_of(&selection.click(1, false, true)), [1, 2, 4, 5]);
    assert_eq!(rows_of(selection.ranges()), [1, 2, 3]);

    assert_eq!(rows_of(&selection.click(8, true, false)), [8]);
    assert_eq!(rows_of(&selection.click(2, true, false)), [2]);
    assert_eq!(rows_of(selection.ranges()), [1, 3, 8]);

    assert_eq!(rows_of(&selection.click(4, true, true)), [2, 4]);
    assert_eq!(rows_of(selection.ranges()), [1, 2, 3, 4, 8]);

    assert!(rows_of(&selection.focus(3)).is_empty());
    assert_eq!(rows_of(&selection.focus(9)), [1, 2, 3, 4, 8, 9]);
    assert_eq!(selection.len(), 1);

    let mut fresh = Selection::default();
    assert_eq!(rows_of(&fresh.click(6, false, true)), [6]);
}

#[test]
fn select_all_and_clear_span_every_row_without_moving_the_anchor() {
    let mut selection = Selection::default();
    selection.click(2, true, false);
    assert_eq!(rows_of(&selection.select_all(5)), [0, 1, 3, 4]);
    assert_eq!(selection.len(), 5);
    assert_eq!(rows_of(&selection.click(4, false, true)), [0, 1]);
    assert_eq!(rows_of(selection.ranges()), [2, 3, 4]);
    assert_eq!(rows_of(&selection.clear()), [2, 3, 4]);
    assert_eq!(selection.len(), 0);
    assert_eq!(rows_of(&selection.click(1, false, true)), [1]);

    let mut library = Selection::default();
    let changed = library.select_all(100_000);
    assert_eq!(changed, SelectionRanges::spanning(0..100_000));
    assert_eq!(library.ranges(), &SelectionRanges::spanning(0..100_000));
    assert_eq!(library.len(), 100_000);
    assert_eq!(library.select_all(0).row_count(), 100_000);
}

#[test]
fn selection_marks_rows_and_resets_with_the_query() {
    let (model, _log) = logging_model();
    model.reset(SongListQuery::default());
    model.apply_count(1, 250);
    model.apply_page(1, 0, page_of(0, PAGE_SIZE));

    assert_eq!(model.select(2, false, false), 1);
    assert_eq!(model.select(150, false, true), 149);
    assert!(model.row_data(2).is_some_and(|row| row.selected));
    assert!(
        model
            .row_data(120)
            .is_some_and(|row| row.selected && !row.loaded)
    );
    assert!(model.row_data(1).is_some_and(|row| !row.selected));
    assert_eq!(model.select(400, false, false), 149);

    let SelectedSongs::Listed(listed) = model.selected_songs() else {
        panic!("a selection over unloaded rows is listed");
    };
    assert_eq!(listed.ranges, SelectionRanges::spanning(2..151));
    let first = model.first_selected();
    assert_eq!(
        first.and_then(|first| first.song).map(|song| song.id),
        Some("s2".into())
    );

    assert_eq!(model.select(5, false, true), 4);
    let SelectedSongs::Loaded(songs) = model.selected_songs() else {
        panic!("a selection over cached rows is loaded");
    };
    let ids: Vec<String> = songs.into_iter().map(|song| song.id).collect();
    assert_eq!(ids, ["s2", "s3", "s4", "s5"]);

    assert_eq!(model.select_all(), 250);
    assert!(model.row_data(249).is_some_and(|row| row.selected));
    assert_eq!(model.clear_selection(), 0);
    assert!(model.row_data(2).is_some_and(|row| !row.selected));

    model.select(1, false, false);
    model.reset(SongListQuery::default());
    assert!(matches!(model.selected_songs(), SelectedSongs::Loaded(songs) if songs.is_empty()));
    assert!(model.first_selected().is_none());
}

#[test]
fn updates_reach_loaded_rows() {
    let (model, _log) = logging_model();
    model.reset(SongListQuery::default());
    model.apply_count(1, 10);
    model.apply_page(1, 0, page_of(0, 10));
    model.update_songs(&["s4".into()], |song| {
        song.favorite = true;
        song.user_rating = Some(4);
    });
    let row = model.row_data(4).unwrap_or_default();
    assert!(row.favorite);
    assert_eq!(row.rating, 4);
    assert!(model.row_data(3).is_some_and(|row| !row.favorite));
}

async fn songs_in(server: &FakeServer, query: &SongListQuery, paging: Paging) -> Vec<Song> {
    server
        .song_list(query, paging)
        .await
        .map(|page| page.items)
        .unwrap_or_default()
}

async fn listed_songs(server: &FakeServer, listed: ListedSongs) -> Vec<Song> {
    let mut chunks = listed.into_chunks(LISTED_CHUNK_SIZE);
    let mut songs = Vec::new();
    while let Ok(Some(chunk)) = chunks.next_songs(server).await {
        songs.extend(chunk);
    }
    songs
}

#[test]
fn filters_feed_the_query() {
    let base = SongListQuery {
        sort: SongSort::Year,
        search: Some("river".into()),
        ..SongListQuery::default()
    };
    let query = Filters {
        favorite_only: true,
        genre_id: Some("genre-2".into()),
        year_from: Some(2000),
        year_to: Some(1990),
    }
    .apply(base.clone());
    assert_eq!(query.favorite, Some(true));
    assert_eq!(query.genre_ids, ["genre-2"]);
    assert_eq!((query.year_from, query.year_to), (Some(1990), Some(2000)));
    assert_eq!(
        (query.sort, query.search.as_deref()),
        (SongSort::Year, Some("river"))
    );

    let cleared = Filters::default().apply(query);
    assert_eq!(cleared, base);

    assert_eq!(parse_year(" 1999 "), Some(1999));
    assert_eq!(parse_year(""), None);
    assert_eq!(parse_year("0"), None);
    assert_eq!(parse_year("abc"), None);
}

#[test]
fn rows_carry_links_favorite_and_rating() {
    let server = FakeServer::new(24);
    let song = Song {
        favorite: true,
        user_rating: Some(3),
        ..server.song_at(13)
    };
    let row = track_row(&song, 13);
    assert_eq!(row.index, 14);
    assert_eq!(row.album_id, "album-1");
    assert!(row.artist_id.starts_with("artist-"));
    assert!(row.favorite);
    assert_eq!(row.rating, 3);
    assert_eq!(sort_for("favorite"), SongSort::Favorited);
    assert_eq!(sort_for("rating"), SongSort::Rating);
}
