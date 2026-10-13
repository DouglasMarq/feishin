use std::hash::Hash;
use std::sync::Arc;

use super::short_key_hasher::ShortKeyMap;

#[derive(Debug, thiserror::Error)]
#[error("a queue track refers to a value missing from the shared tables")]
pub(super) struct MissingSharedValue;

pub(super) struct SharedTable<'queue, Value: ?Sized> {
    values: Vec<&'queue Value>,
    indexes_by_address: ShortKeyMap<usize, usize>,
    indexes_by_content: ShortKeyMap<&'queue Value, usize>,
}

fn address_of<Value: ?Sized>(value: &Arc<Value>) -> usize {
    Arc::as_ptr(value).cast::<u8>().addr()
}

impl<Value: ?Sized> Default for SharedTable<'_, Value> {
    fn default() -> Self {
        Self {
            values: Vec::new(),
            indexes_by_address: ShortKeyMap::default(),
            indexes_by_content: ShortKeyMap::default(),
        }
    }
}

impl<'queue, Value: Eq + Hash + ?Sized> SharedTable<'queue, Value> {
    pub(super) fn values(&self) -> &[&'queue Value] {
        &self.values
    }

    pub(super) fn add(&mut self, value: &'queue Arc<Value>) {
        let address = address_of(value);
        if self.indexes_by_address.contains_key(&address) {
            return;
        }
        let index = self.content_index(value);
        self.indexes_by_address.insert(address, index);
    }

    pub(super) fn index_of(&self, value: &Arc<Value>) -> Result<usize, MissingSharedValue> {
        self.indexes_by_address
            .get(&address_of(value))
            .copied()
            .ok_or(MissingSharedValue)
    }

    pub(super) fn optional_index_of(
        &self,
        value: Option<&Arc<Value>>,
    ) -> Result<Option<usize>, MissingSharedValue> {
        value.map(|present| self.index_of(present)).transpose()
    }

    fn content_index(&mut self, value: &'queue Value) -> usize {
        let next_index = self.values.len();
        let index = *self.indexes_by_content.entry(value).or_insert(next_index);
        if index == next_index {
            self.values.push(value);
        }
        index
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_values_in_separate_allocations_share_one_entry() {
        let first: Arc<str> = Arc::from("Album");
        let copy: Arc<str> = Arc::from("Album");
        let other: Arc<str> = Arc::from("Other");
        let mut table = SharedTable::default();
        for value in [&first, &copy, &other, &first] {
            table.add(value);
        }
        assert_eq!(table.values(), ["Album", "Other"]);
        assert_eq!(table.index_of(&copy).ok(), Some(0));
        assert_eq!(table.index_of(&other).ok(), Some(1));
        assert!(table.index_of(&Arc::from("Album")).is_err());
    }
}
