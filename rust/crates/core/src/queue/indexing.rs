use super::Queue;

impl Queue {
    pub(super) fn reindex_all(&mut self) {
        self.uid_indexes = self
            .items
            .iter()
            .enumerate()
            .map(|(item_index, item)| (item.uid, item_index))
            .collect();
    }

    pub(super) fn reindex_from(&mut self, first_index: usize) {
        let shifted_entries = self
            .items
            .iter()
            .enumerate()
            .skip(first_index)
            .map(|(item_index, item)| (item.uid, item_index));
        self.uid_indexes.extend(shifted_entries);
    }

    pub(super) fn bump_revision(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
}
