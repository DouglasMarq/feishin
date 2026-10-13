use feishin_core::model::Song;

use crate::demo::test_song;
use crate::list_stream::pages::{PageNumber, STREAM_PAGE_SIZE, keep_listed_rows};
use crate::list_stream::rows::{ListRow, RowRanges};

#[test]
fn ranges_are_sorted_merged_and_empty_ones_dropped() {
    let rows = RowRanges::from_ranges(vec![10..20, 3..5, 15..30, 7..7, 30..31]);
    assert_eq!(rows, RowRanges::from_ranges(vec![3..5, 10..31]));
    assert_eq!(rows.first_row(), Some(ListRow(3)));
    assert_eq!(rows.end(), 31);
    assert_eq!(rows.row_count(), 23);
    assert!(rows.contains(ListRow(4)) && rows.contains(ListRow(30)));
    assert!(!rows.contains(ListRow(5)) && !rows.contains(ListRow(31)));
    assert!(rows.intersects(&(0..4)) && rows.intersects(&(25..100)));
    assert!(!rows.intersects(&(5..10)) && !rows.intersects(&(31..500)));
    assert_eq!(RowRanges::from_ranges(Vec::new()).first_row(), None);
}

#[test]
fn rows_are_counted_clamped_and_indexed() {
    let rows = RowRanges::from_ranges(vec![3..5, 10..31]);
    assert_eq!(rows.nth_row(0), Some(ListRow(3)));
    assert_eq!(rows.nth_row(2), Some(ListRow(10)));
    assert_eq!(rows.nth_row(22), Some(ListRow(30)));
    assert_eq!(rows.nth_row(23), None);
    assert_eq!(rows.clamped(12), RowRanges::from_ranges(vec![3..5, 10..12]));
    assert_eq!(RowRanges::whole_list().clamped(40).row_count(), 40);
    assert_eq!(RowRanges::whole_list().row_count(), usize::MAX);
}

#[test]
fn selected_rows_compress_into_ranges() {
    assert_eq!(RowRanges::covering(&[9, 1, 2, 3, 7, 8]), [1..4, 7..10]);
    assert!(RowRanges::covering(&[]).is_empty());
}

#[test]
fn pages_cover_fixed_row_windows() {
    let page = PageNumber::of(ListRow(1234));
    assert_eq!(page.first_row(), ListRow(1000));
    assert_eq!(page.rows(), 1000..1000 + STREAM_PAGE_SIZE);
    assert_eq!(page.paging().start_index, 1000);
    let following: Vec<ListRow> = page
        .following()
        .take(2)
        .map(PageNumber::first_row)
        .collect();
    assert_eq!(following, [ListRow(1500), ListRow(2000)]);
    let preceding: Vec<ListRow> = page
        .preceding_down_to(PageNumber::of(ListRow(0)))
        .map(PageNumber::first_row)
        .collect();
    assert_eq!(preceding, [ListRow(500), ListRow(0)]);
}

#[test]
fn only_listed_rows_of_a_page_are_kept() {
    let songs: Vec<Song> = (500..510)
        .map(|row| test_song(&format!("song-{row}"), 1000))
        .collect();
    let listed = RowRanges::from_ranges(vec![502..504, 508..600]);
    let kept = keep_listed_rows(PageNumber::of(ListRow(500)), songs, &listed);
    let ids: Vec<&str> = kept.songs.iter().map(|song| song.id.as_str()).collect();
    assert_eq!(ids, ["song-502", "song-503", "song-508", "song-509"]);
    assert_eq!(
        kept.rows,
        [ListRow(502), ListRow(503), ListRow(508), ListRow(509)]
    );
}
