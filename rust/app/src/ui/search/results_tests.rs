use feishin_core::model::SearchResults;
use slint::Model;

use super::query::SEARCH_LIMIT;
use super::results::Results;
use super::searches::More;
use super::section::MORE_PAGE;
use super::test_support::numbered_songs;
use crate::ui::SearchKind;

fn found_songs(results: &Results, count: usize) {
    let mut searches = results.searches.borrow_mut();
    let generation = searches.begin(Some("song".into()), false).unwrap();
    let found = SearchResults {
        songs: numbered_songs(count),
        albums: Vec::new(),
        artists: Vec::new(),
    };
    assert!(searches.finish(generation, Some(found), SEARCH_LIMIT));
}

fn more_songs(results: &Results, count: usize) -> usize {
    let mut searches = results.searches.borrow_mut();
    let request = searches.more(SearchKind::Songs).unwrap();
    let applied = searches
        .apply_more(
            request.generation,
            request.paging,
            More::Songs(numbered_songs(count)),
        )
        .unwrap();
    applied.first_new_index()
}

#[test]
fn song_rows_are_built_on_demand_from_the_found_songs() {
    let results = Results::default();
    assert_eq!(results.row_count(), 0);

    found_songs(&results, SEARCH_LIMIT);
    assert_eq!(results.row_count(), 0);
    results.show_songs(0);
    assert_eq!(results.row_count(), SEARCH_LIMIT);
    let row = results.row_data(3).unwrap();
    assert_eq!((row.id.as_str(), row.index), ("s3", 4));
    assert!(results.row_data(SEARCH_LIMIT).is_none());

    let start = more_songs(&results, SEARCH_LIMIT + MORE_PAGE);
    results.show_songs(start);
    assert_eq!(results.row_count(), SEARCH_LIMIT + MORE_PAGE);

    let start = more_songs(&results, MORE_PAGE);
    assert_eq!(start, SEARCH_LIMIT + MORE_PAGE);
    results.show_songs(start);
    assert_eq!(results.row_count(), SEARCH_LIMIT + 2 * MORE_PAGE);
    let last = results.row_data(SEARCH_LIMIT + 2 * MORE_PAGE - 1).unwrap();
    assert_eq!(last.index, 120);
}
