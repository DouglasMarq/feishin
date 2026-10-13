use std::sync::Arc;

use super::{FIRST_POSITION, Queue, QueueItem, Uid};
use crate::model::Song;

impl Queue {
    pub fn play_now(
        &mut self,
        songs: Vec<Song>,
        start_song_id: Option<&str>,
    ) -> Option<&QueueItem> {
        let start_index =
            start_song_id.and_then(|song_id| songs.iter().position(|song| song.id == song_id));
        self.play_now_at(songs, start_index)
    }

    pub fn play_now_at(
        &mut self,
        songs: Vec<Song>,
        start_index: Option<usize>,
    ) -> Option<&QueueItem> {
        self.interner.clear();
        self.items = self.create_items(&songs);
        self.reindex_all();
        let start_index = start_index.filter(|&index| index < self.items.len());
        let start_position = self.start_position_after_replacing(start_index);
        self.current_uid = self.uid_at_position(start_position);
        self.bump_revision();
        self.current()
    }

    pub fn add_next(&mut self, songs: Vec<Song>) {
        if songs.is_empty() {
            return;
        }
        let added_count = songs.len();
        let insert_index = self.current_index().map_or(0, |index| index + 1);
        let insert_position = self.current_position().map_or(0, |position| position + 1);
        let new_items = self.create_items(&songs);
        self.items.splice(insert_index..insert_index, new_items);
        self.reindex_from(insert_index);
        self.insert_into_shuffled_order(insert_index, insert_position, added_count);
        self.bump_revision();
    }

    pub fn add_last(&mut self, songs: Vec<Song>) {
        let first_new_index = self.items.len();
        let new_items = self.create_items(&songs);
        self.items.extend(new_items);
        self.reindex_from(first_new_index);
        self.append_to_shuffled_order(first_new_index);
        self.bump_revision();
    }

    fn append_to_shuffled_order(&mut self, first_new_index: usize) {
        let Some(mut play_order) = self.shuffled_play_order.take() else {
            return;
        };
        let new_indexes = self.shuffled_indexes((first_new_index..self.items.len()).collect());
        play_order.extend(new_indexes);
        self.shuffled_play_order = Some(play_order);
    }

    pub(super) fn allocate_uid(&mut self) -> Uid {
        let uid = self.next_uid;
        self.next_uid = self.next_uid.saturating_add(1);
        uid
    }

    pub(super) fn create_items(&mut self, songs: &[Song]) -> Vec<QueueItem> {
        songs
            .iter()
            .map(|song| QueueItem {
                uid: self.allocate_uid(),
                track: Arc::new(self.interner.track_of(song)),
            })
            .collect()
    }

    fn start_position_after_replacing(&mut self, start_index: Option<usize>) -> usize {
        if !self.shuffle() {
            return start_index.unwrap_or(FIRST_POSITION);
        }
        self.shuffled_play_order = Some(self.shuffled_order(start_index));
        FIRST_POSITION
    }

    pub(super) fn insert_into_shuffled_order(
        &mut self,
        insert_index: usize,
        insert_position: usize,
        added_count: usize,
    ) {
        let Some(play_order) = self.shuffled_play_order.take() else {
            return;
        };
        let mut shifted_order = shift_indexes_from(play_order, insert_index, added_count);
        let new_indexes =
            self.shuffled_indexes((insert_index..insert_index + added_count).collect());
        shifted_order.splice(insert_position..insert_position, new_indexes);
        self.shuffled_play_order = Some(shifted_order);
    }
}

pub(super) fn shift_indexes_from(
    mut play_order: Vec<usize>,
    first_shifted: usize,
    shift: usize,
) -> Vec<usize> {
    play_order
        .iter_mut()
        .filter(|item_index| **item_index >= first_shifted)
        .for_each(|item_index| *item_index += shift);
    play_order
}
