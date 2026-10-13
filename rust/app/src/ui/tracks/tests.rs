mod headless;

use std::cell::RefCell;
use std::rc::Rc;

use feishin_api::MusicServer;
use feishin_core::model::{Paging, Song, SongListQuery, SongSort};
use slint::Model;

use super::model::{PAGE_SIZE, Request, TracksModel};
use super::query::{Filters, parse_year, sort_for};
use super::row::track_row;
use super::selection::{Selection, SelectionSnapshot};
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
    assert_eq!(drain(&log), [(1, Some(1))]);

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

#[test]
fn clicks_follow_desktop_selection_rules() {
    let mut selection = Selection::default();
    assert_eq!(selection.click(3, false, false), [3]);
    assert_eq!(selection.click(5, false, true), [4, 5]);
    assert_eq!(selection.rows().collect::<Vec<_>>(), [3, 4, 5]);

    assert_eq!(selection.click(1, false, true), [1, 2, 4, 5]);
    assert_eq!(selection.rows().collect::<Vec<_>>(), [1, 2, 3]);

    assert_eq!(selection.click(8, true, false), [8]);
    assert_eq!(selection.click(2, true, false), [2]);
    assert_eq!(selection.rows().collect::<Vec<_>>(), [1, 3, 8]);

    assert_eq!(selection.click(4, true, true), [2, 4]);
    assert_eq!(selection.rows().collect::<Vec<_>>(), [1, 2, 3, 4, 8]);

    assert!(selection.focus(3).is_empty());
    assert_eq!(selection.focus(9), [1, 2, 3, 4, 8, 9]);
    assert_eq!(selection.len(), 1);

    let mut fresh = Selection::default();
    assert_eq!(fresh.click(6, false, true), [6]);
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

    let snapshot = model.selection();
    assert_eq!(
        snapshot.missing_pages().into_iter().collect::<Vec<_>>(),
        [1]
    );
    assert!(!snapshot.is_loaded());
    assert_eq!(
        model.first_selected().map(|song| song.id),
        Some("s2".into())
    );

    model.reset(SongListQuery::default());
    assert_eq!(model.selection().rows.len(), 0);
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

#[tokio::test]
async fn selections_over_unloaded_pages_are_fetched() {
    let server = FakeServer::new(500);
    let query = SongListQuery::default();
    let first_page = Paging {
        start_index: 0,
        limit: PAGE_SIZE,
    };
    let first = songs_in(&server, &query, first_page).await;
    let snapshot = SelectionSnapshot {
        query: query.clone(),
        rows: vec![(5, first.get(5).cloned()), (230, None), (231, None)],
    };
    let songs = snapshot.resolve(&server).await.unwrap_or_default();
    let unloaded_rows = Paging {
        start_index: 230,
        limit: 2,
    };
    let expected = songs_in(&server, &query, unloaded_rows).await;
    assert_eq!(songs.len(), 3);
    assert_eq!(songs[0].id, first[5].id);
    assert_eq!(songs[1..], expected[..]);
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
