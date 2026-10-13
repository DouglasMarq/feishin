use std::fmt;

use feishin_core::queue::{QueueItem, QueueSnapshot};
use serde::de::{self, DeserializeSeed, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

use super::StoredQueue;
use super::compact::{CompactTrack, QUEUE_FORMAT};
use super::decoded_tables::DecodedTables;
use crate::Result;

const QUEUE_FIELDS: &[&str] = &[
    "strings",
    "artist_lists",
    "tracks",
    "shuffled",
    "current_uid",
    "repeat",
    "next_uid",
    "position_ms",
];
const MAX_PREALLOCATED_TRACKS: usize = 1 << 20;
const TRACKS_FIELD_INDEX: usize = 2;

struct DecodedQueue(StoredQueue);

struct QueueVisitor;

#[derive(Clone, Copy)]
struct TrackRowsSeed<'tables> {
    tables: &'tables DecodedTables,
}

pub(super) fn decode(bytes: &[u8]) -> Result<StoredQueue> {
    QUEUE_FORMAT
        .decode::<DecodedQueue>(bytes)
        .map(|decoded| decoded.0)
}

fn next_field<'de, Fields: SeqAccess<'de>, Value: Deserialize<'de>>(
    fields: &mut Fields,
    index: usize,
) -> std::result::Result<Value, Fields::Error> {
    fields
        .next_element()?
        .ok_or_else(|| de::Error::invalid_length(index, &"a compact queue"))
}

impl<'de> Deserialize<'de> for DecodedQueue {
    fn deserialize<QueueDeserializer: Deserializer<'de>>(
        deserializer: QueueDeserializer,
    ) -> std::result::Result<Self, QueueDeserializer::Error> {
        deserializer.deserialize_struct("CompactQueue", QUEUE_FIELDS, QueueVisitor)
    }
}

impl<'de> Visitor<'de> for QueueVisitor {
    type Value = DecodedQueue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a compact queue")
    }

    fn visit_seq<Fields: SeqAccess<'de>>(
        self,
        mut fields: Fields,
    ) -> std::result::Result<DecodedQueue, Fields::Error> {
        let tables = DecodedTables::new(next_field(&mut fields, 0)?, next_field(&mut fields, 1)?);
        let items = fields
            .next_element_seed(TrackRowsSeed { tables: &tables })?
            .ok_or_else(|| de::Error::invalid_length(TRACKS_FIELD_INDEX, &self))?;
        let snapshot = QueueSnapshot {
            items,
            shuffled: next_field(&mut fields, 3)?,
            current_uid: next_field(&mut fields, 4)?,
            repeat: next_field(&mut fields, 5)?,
            next_uid: next_field(&mut fields, 6)?,
        };
        let position_ms = next_field(&mut fields, 7)?;
        Ok(DecodedQueue(StoredQueue {
            snapshot,
            position_ms,
        }))
    }
}

impl<'de> DeserializeSeed<'de> for TrackRowsSeed<'_> {
    type Value = Vec<QueueItem>;

    fn deserialize<RowsDeserializer: Deserializer<'de>>(
        self,
        deserializer: RowsDeserializer,
    ) -> std::result::Result<Vec<QueueItem>, RowsDeserializer::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for TrackRowsSeed<'_> {
    type Value = Vec<QueueItem>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("compact queue tracks")
    }

    fn visit_seq<Rows: SeqAccess<'de>>(
        self,
        mut rows: Rows,
    ) -> std::result::Result<Vec<QueueItem>, Rows::Error> {
        let expected = rows.size_hint().unwrap_or(0);
        let mut items = Vec::with_capacity(expected.min(MAX_PREALLOCATED_TRACKS));
        while let Some(row) = rows.next_element::<CompactTrack<String>>()? {
            items.push(self.tables.item_of(row).map_err(de::Error::custom)?);
        }
        Ok(items)
    }
}
