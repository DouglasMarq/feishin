use super::super::selection_ranges::SelectionRanges;
use super::rows_of;

fn ranges(list: &[std::ops::Range<usize>]) -> SelectionRanges {
    SelectionRanges::from_ranges(list.to_vec())
}

#[test]
fn ranges_are_sorted_and_merged() {
    let merged = ranges(&[8..9, 0..2, 1..4, 4..5, 6..6]);
    assert_eq!(merged.clone().into_ranges(), [0..5, 8..9]);
    assert_eq!(merged.row_count(), 6);
    assert_eq!(merged.first_row(), Some(0));
    assert_eq!(rows_of(&merged), [0, 1, 2, 3, 4, 8]);
    assert!(merged.contains(4) && merged.contains(8));
    assert!(!merged.contains(5) && !merged.contains(9));
    assert_eq!(SelectionRanges::default().first_row(), None);
}

#[test]
fn inserts_join_touching_and_overlapping_ranges() {
    let mut selection = ranges(&[2..4, 10..12, 20..22]);
    selection.insert(4..5);
    assert_eq!(selection.clone().into_ranges(), [2..5, 10..12, 20..22]);
    selection.insert(6..8);
    assert_eq!(
        selection.clone().into_ranges(),
        [2..5, 6..8, 10..12, 20..22]
    );
    selection.insert(7..21);
    assert_eq!(selection.clone().into_ranges(), [2..5, 6..22]);
    selection.insert(0..1);
    selection.insert(30..30);
    assert_eq!(selection.clone().into_ranges(), [0..1, 2..5, 6..22]);
    selection.insert(0..40);
    assert_eq!(selection, SelectionRanges::spanning(0..40));
}

#[test]
fn removing_and_toggling_split_ranges() {
    let mut selection = SelectionRanges::spanning(0..10);
    selection.remove(4);
    assert_eq!(selection.clone().into_ranges(), [0..4, 5..10]);
    selection.remove(0);
    selection.remove(9);
    selection.remove(42);
    assert_eq!(selection.clone().into_ranges(), [1..4, 5..9]);
    selection.toggle(4);
    assert_eq!(selection, SelectionRanges::spanning(1..9));
    selection.toggle(8);
    selection.toggle(1);
    assert_eq!(selection, SelectionRanges::spanning(2..8));
    selection.toggle(2);
    selection.toggle(7);
    selection.toggle(3);
    assert_eq!(selection, SelectionRanges::spanning(4..7));
}

#[test]
fn symmetric_difference_lists_changed_rows() {
    let before = ranges(&[0..5, 10..15]);
    let after = SelectionRanges::spanning(3..12);
    let changed = before.symmetric_difference(&after);
    assert_eq!(changed.into_ranges(), [0..3, 5..10, 12..15]);
    assert_eq!(
        after.symmetric_difference(&before).into_ranges(),
        [0..3, 5..10, 12..15]
    );
    assert_eq!(before.symmetric_difference(&before).row_count(), 0);
    let whole = SelectionRanges::spanning(0..100_000);
    assert_eq!(
        SelectionRanges::default().symmetric_difference(&whole),
        whole
    );
}
