use feishin_core::queue::{Advance, Queue, Uid};
use slint::Model;

use super::{QueueModel, ViewChange};
use crate::demo::test_song;

fn playing_queue(count: usize) -> Queue {
    let mut queue = Queue::with_seed(7);
    let songs = (0..count)
        .map(|index| test_song(&format!("s{index}"), 61_000))
        .collect();
    queue.play_now(songs, Some("s0"));
    queue
}

fn shown_uids(model: &QueueModel) -> Vec<Uid> {
    (0..model.row_count())
        .filter_map(|row| model.row_data(row))
        .filter_map(|row| row.uid.parse().ok())
        .collect()
}

fn current_rows(model: &QueueModel) -> Vec<usize> {
    (0..model.row_count())
        .filter(|&row| model.row_data(row).is_some_and(|data| data.current))
        .collect()
}

#[test]
fn drag_targets_map_to_move_items_arguments() {
    let model = QueueModel::default();
    model.set_view(playing_queue(4).view());
    assert_eq!(model.move_target(3, 1), Some((4, Some(2))));
    assert_eq!(model.move_target(0, 2), Some((1, Some(4))));
    assert_eq!(model.move_target(0, 3), Some((1, None)));
    assert_eq!(model.move_target(2, 2), None);
    assert_eq!(model.move_target(9, 0), None);
}

#[test]
fn rows_are_built_on_demand_from_the_view() {
    let model = QueueModel::default();
    model.set_view(playing_queue(3).view());
    assert_eq!(model.row_count(), 3);
    let row = model.row_data(1).unwrap_or_default();
    assert_eq!(row.uid, "2");
    assert_eq!(row.title, "s1");
    assert_eq!(row.duration, "1:01");
    assert!(!row.current);
    assert!(model.row_data(3).is_none());
}

#[test]
fn track_changes_only_move_the_highlight() {
    let model = QueueModel::default();
    let mut queue = playing_queue(4);
    assert_eq!(model.set_view(queue.view()), ViewChange::LayoutReplaced);
    queue.next(Advance::Manual);
    assert_eq!(model.set_view(queue.view()), ViewChange::CurrentMoved);
    assert_eq!(current_rows(&model), vec![1]);
    assert_eq!(model.shared_view().borrow().current_position(), Some(1));
}

#[test]
fn edits_replace_the_layout() {
    let model = QueueModel::default();
    let mut queue = playing_queue(4);
    model.set_view(queue.view());
    queue.remove(&[3]);
    assert_eq!(model.set_view(queue.view()), ViewChange::LayoutReplaced);
    assert_eq!(shown_uids(&model), vec![1, 2, 4]);
}

#[test]
fn shuffled_queue_is_shown_in_play_order() {
    let model = QueueModel::default();
    let mut queue = playing_queue(20);
    queue.set_shuffle(true);
    queue.next(Advance::Manual);
    model.set_view(queue.view());
    let play_order: Vec<Uid> = queue.display_order().iter().map(|item| item.uid).collect();
    assert_eq!(shown_uids(&model), play_order);
    assert_eq!(current_rows(&model), vec![1]);
}
