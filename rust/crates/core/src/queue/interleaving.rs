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

pub(super) struct AddedItems {
    pub(super) first_index: usize,
    pub(super) count: usize,
    pub(super) fallback_position: usize,
}

impl Queue {
    pub(super) fn interleave_into_shuffled_order(
        &mut self,
        added: AddedItems,
        is_own: &dyn Fn(Uid) -> bool,
    ) {
        let Some(play_order) = self.shuffled_play_order.take() else {
            return;
        };
        let mut merged = shift_indexes_from(play_order, added.first_index, added.count);
        let first_upcoming = self.first_upcoming_position(&merged);
        let upcoming = self.classify(merged.split_off(first_upcoming), is_own);
        let added_range = added.first_index..added.first_index + added.count;
        let added_indexes = self.shuffled_indexes(added_range.collect());
        let placed = match upcoming.own_count {
            0 => {
                let offset = added.fallback_position.saturating_sub(first_upcoming);
                place_at(upcoming, added_indexes, offset)
            }
            own_count => {
                let slots = self.shuffled_slots(own_count, added.count);
                interleave(upcoming, added_indexes, slots)
            }
        };
        merged.extend(placed);
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

fn place_at(upcoming: Upcoming, added: Vec<usize>, offset: usize) -> Vec<usize> {
    let mut placed: Vec<usize> = upcoming
        .entries
        .into_iter()
        .map(|(item_index, _)| item_index)
        .collect();
    let offset = offset.min(placed.len());
    placed.splice(offset..offset, added);
    placed
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
