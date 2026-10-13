use std::ops::Range;

use super::{Queue, Uid};
use crate::model::Song;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertAnchor {
    Start,
    End,
    After(Uid),
    Before(Uid),
}

impl Queue {
    pub fn contains(&self, uid: Uid) -> bool {
        self.uid_indexes.contains_key(&uid)
    }

    pub fn insert_at_anchor(&mut self, songs: &[Song], anchor: InsertAnchor) -> Range<Uid> {
        let insert_index = self.anchor_index(anchor);
        let insert_position = self.anchor_position(anchor);
        let inserted = self.splice_songs(insert_index, songs);
        self.insert_into_shuffled_order(insert_index, insert_position, songs.len());
        inserted
    }

    pub fn insert_interleaved(
        &mut self,
        songs: &[Song],
        anchor: InsertAnchor,
        is_own: impl Fn(Uid) -> bool,
    ) -> Range<Uid> {
        let insert_index = self.anchor_index(anchor);
        let inserted = self.splice_songs(insert_index, songs);
        self.interleave_into_shuffled_order(insert_index, songs.len(), &is_own);
        inserted
    }

    fn splice_songs(&mut self, insert_index: usize, songs: &[Song]) -> Range<Uid> {
        let first_uid = self.next_uid;
        if songs.is_empty() {
            return first_uid..first_uid;
        }
        let new_items = self.create_items(songs);
        self.items.splice(insert_index..insert_index, new_items);
        self.reindex_from(insert_index);
        self.bump_revision();
        first_uid..self.next_uid
    }

    fn anchor_index(&self, anchor: InsertAnchor) -> usize {
        let item_count = self.items.len();
        match anchor {
            InsertAnchor::Start => 0,
            InsertAnchor::End => item_count,
            InsertAnchor::After(uid) => self.index_of(uid).map_or(item_count, |index| index + 1),
            InsertAnchor::Before(uid) => self.index_of(uid).unwrap_or(0),
        }
    }

    fn anchor_position(&self, anchor: InsertAnchor) -> usize {
        let item_count = self.items.len();
        let position_of_uid = |uid| {
            self.index_of(uid)
                .and_then(|item_index| self.position_of_index(item_index))
        };
        match anchor {
            InsertAnchor::Start => 0,
            InsertAnchor::End => item_count,
            InsertAnchor::After(uid) => {
                position_of_uid(uid).map_or(item_count, |position| position + 1)
            }
            InsertAnchor::Before(uid) => position_of_uid(uid).unwrap_or(0),
        }
    }
}
