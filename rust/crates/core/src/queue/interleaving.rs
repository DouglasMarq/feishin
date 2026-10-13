use std::iter::Peekable;

use super::insertion::shift_indexes_from;
use super::{Queue, Uid};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ownership {
    Own,
    Foreign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
    Existing,
    Added,
}

struct Upcoming {
    entries: Vec<(usize, Ownership)>,
    own_count: usize,
}

impl Queue {
    pub(super) fn interleave_into_shuffled_order(
        &mut self,
        insert_index: usize,
        added_count: usize,
        is_own: &dyn Fn(Uid) -> bool,
    ) {
        let Some(play_order) = self.shuffled_play_order.take() else {
            return;
        };
        let mut merged = shift_indexes_from(play_order, insert_index, added_count);
        let upcoming_order = merged.split_off(self.first_upcoming_position(&merged));
        let upcoming = self.classify(upcoming_order, is_own);
        let added = self.shuffled_indexes((insert_index..insert_index + added_count).collect());
        let slots = self.shuffled_slots(upcoming.own_count, added_count);
        merged.extend(interleave(upcoming, added, slots));
        self.shuffled_play_order = Some(merged);
    }

    fn first_upcoming_position(&self, play_order: &[usize]) -> usize {
        self.current_index()
            .and_then(|current| play_order.iter().position(|&index| index == current))
            .map_or(0, |position| position + 1)
    }

    fn classify(&self, upcoming_order: Vec<usize>, is_own: &dyn Fn(Uid) -> bool) -> Upcoming {
        let entries: Vec<(usize, Ownership)> = upcoming_order
            .into_iter()
            .map(|item_index| {
                let own = self.uid_at_index(item_index).is_some_and(is_own);
                let ownership = match own {
                    true => Ownership::Own,
                    false => Ownership::Foreign,
                };
                (item_index, ownership)
            })
            .collect();
        let own_count = entries
            .iter()
            .filter(|(_, ownership)| *ownership == Ownership::Own)
            .count();
        Upcoming { entries, own_count }
    }

    fn shuffled_slots(&mut self, existing_count: usize, added_count: usize) -> Vec<Slot> {
        let mut slots = vec![Slot::Added; added_count];
        slots.resize(existing_count + added_count, Slot::Existing);
        self.random_generator.shuffle(&mut slots);
        slots
    }
}

fn interleave(upcoming: Upcoming, added: Vec<usize>, slots: Vec<Slot>) -> Vec<usize> {
    let mut merged = Vec::with_capacity(upcoming.entries.len() + added.len());
    let mut existing = upcoming.entries.into_iter().peekable();
    let mut added = added.into_iter();
    let mut own_taken = 0;
    take_foreign(&mut existing, &mut merged);
    for slot in slots {
        match slot {
            Slot::Added => merged.extend(added.next()),
            Slot::Existing => {
                merged.extend(existing.next().map(|(item_index, _)| item_index));
                own_taken += 1;
                if own_taken < upcoming.own_count {
                    take_foreign(&mut existing, &mut merged);
                }
            }
        }
    }
    merged.extend(existing.map(|(item_index, _)| item_index));
    merged.extend(added);
    merged
}

fn take_foreign(
    existing: &mut Peekable<impl Iterator<Item = (usize, Ownership)>>,
    merged: &mut Vec<usize>,
) {
    while let Some((item_index, _)) =
        existing.next_if(|(_, ownership)| *ownership == Ownership::Foreign)
    {
        merged.push(item_index);
    }
}
