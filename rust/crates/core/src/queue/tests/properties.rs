use proptest::prelude::*;

use super::support::{assert_invariants, item_ids, numbered_songs};
use crate::queue::{Advance, Queue, RepeatMode, Uid};

#[derive(Debug, Clone)]
enum Operation {
    PlayNow(usize, Option<usize>),
    AddNext(usize),
    AddLast(usize),
    Next(Advance),
    Previous,
    PlayAt(usize),
    Remove(Vec<usize>),
    Move(Vec<usize>, Option<usize>),
    Shuffle(bool),
    Repeat(RepeatMode),
    Clear,
    Restore,
}

fn advance_strategy() -> impl Strategy<Value = Advance> {
    prop_oneof![Just(Advance::Auto), Just(Advance::Manual)]
}

fn repeat_strategy() -> impl Strategy<Value = RepeatMode> {
    prop_oneof![
        Just(RepeatMode::None),
        Just(RepeatMode::All),
        Just(RepeatMode::One)
    ]
}

fn item_indexes_strategy() -> impl Strategy<Value = Vec<usize>> {
    proptest::collection::vec(0..16usize, 0..4)
}

fn operation_strategy() -> impl Strategy<Value = Operation> {
    prop_oneof![
        (0..12usize, proptest::option::of(0..12usize))
            .prop_map(|(count, start)| Operation::PlayNow(count, start)),
        (0..4usize).prop_map(Operation::AddNext),
        (0..4usize).prop_map(Operation::AddLast),
        advance_strategy().prop_map(Operation::Next),
        Just(Operation::Previous),
        (0..16usize).prop_map(Operation::PlayAt),
        item_indexes_strategy().prop_map(Operation::Remove),
        (item_indexes_strategy(), proptest::option::of(0..16usize))
            .prop_map(|(moved, before)| Operation::Move(moved, before)),
        any::<bool>().prop_map(Operation::Shuffle),
        repeat_strategy().prop_map(Operation::Repeat),
        Just(Operation::Clear),
        Just(Operation::Restore),
    ]
}

fn uid_at(queue: &Queue, item_index: usize) -> Uid {
    queue
        .items()
        .get(item_index)
        .map_or(u64::MAX, |item| item.uid)
}

fn uids_at(queue: &Queue, item_indexes: &[usize]) -> Vec<Uid> {
    item_indexes
        .iter()
        .map(|&item_index| uid_at(queue, item_index))
        .collect()
}

fn apply_operation(queue: &mut Queue, operation: &Operation) {
    match operation {
        Operation::PlayNow(count, start) => {
            let start_id = start.map(|start_index| format!("s{start_index}"));
            queue.play_now(numbered_songs(*count), start_id.as_deref());
        }
        Operation::AddNext(count) => queue.add_next(numbered_songs(*count)),
        Operation::AddLast(count) => queue.add_last(numbered_songs(*count)),
        Operation::Next(advance) => {
            queue.next(*advance);
        }
        Operation::Previous => {
            queue.previous();
        }
        Operation::PlayAt(item_index) => {
            queue.play_uid(uid_at(queue, *item_index));
        }
        Operation::Remove(item_indexes) => queue.remove(&uids_at(queue, item_indexes)),
        Operation::Move(item_indexes, before) => {
            let uids = uids_at(queue, item_indexes);
            let before_uid = before.map(|before_index| uid_at(queue, before_index));
            queue.move_items(&uids, before_uid);
        }
        Operation::Shuffle(is_shuffled) => queue.set_shuffle(*is_shuffled),
        Operation::Repeat(mode) => queue.set_repeat(*mode),
        Operation::Clear => queue.clear(),
        Operation::Restore => *queue = Queue::from_snapshot(queue.snapshot()),
    }
}

fn keeps_original_order(operation: &Operation) -> bool {
    matches!(
        operation,
        Operation::Move(..) | Operation::Next(_) | Operation::Previous | Operation::PlayAt(_)
    )
}

proptest! {
    #[test]
    fn random_operations_keep_invariants(
        seed in any::<u64>(),
        operations in proptest::collection::vec(operation_strategy(), 0..40)
    ) {
        let mut queue = Queue::with_seed(seed);
        for operation in &operations {
            let items_before: Vec<String> = item_ids(&queue);
            let was_shuffled = queue.is_shuffled();
            apply_operation(&mut queue, operation);
            assert_invariants(&queue);

            let peeked = queue.peek_next().cloned();
            prop_assert_eq!(queue.clone().next(Advance::Auto).cloned(), peeked);

            if was_shuffled && queue.is_shuffled() && keeps_original_order(operation) {
                prop_assert_eq!(item_ids(&queue), items_before);
            }
            if !queue.is_empty() && matches!(operation, Operation::PlayNow(..)) {
                prop_assert!(queue.current().is_some());
            }
        }
    }
}
