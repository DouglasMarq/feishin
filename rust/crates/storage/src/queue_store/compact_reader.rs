use std::fmt;
use std::marker::PhantomData;

use feishin_core::queue::{QueueItem, QueueSnapshot};
use serde::de::{self, DeserializeOwned, DeserializeSeed, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

use super::StoredQueue;
use super::codec::FileFormat;
use super::compact::{CompactTrack, CompactTrackV1, QUEUE_FORMAT, QUEUE_FORMAT_V1};
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

trait CompactRow: DeserializeOwned + Into<CompactTrack<String>> {}

impl<Row: DeserializeOwned + Into<CompactTrack<String>>> CompactRow for Row {}

struct DecodedQueue<Row> {
    stored: StoredQueue,
    row: PhantomData<Row>,
}

struct QueueVisitor<Row>(PhantomData<Row>);

struct TrackRowsSeed<'tables, Row> {
    tables: &'tables DecodedTables,
    row: PhantomData<Row>,
}

pub(super) fn decode(bytes: &[u8]) -> Result<StoredQueue> {
    match bytes.starts_with(&QUEUE_FORMAT_V1.magic) {
        true => decode_rows::<CompactTrackV1<String>>(QUEUE_FORMAT_V1, bytes),
        false => decode_rows::<CompactTrack<String>>(QUEUE_FORMAT, bytes),
    }
}

fn decode_rows<Row: CompactRow>(format: FileFormat, bytes: &[u8]) -> Result<StoredQueue> {
    format
        .decode::<DecodedQueue<Row>>(bytes)
        .map(|decoded| decoded.stored)
}

fn next_field<'de, Fields: SeqAccess<'de>, Value: Deserialize<'de>>(
    fields: &mut Fields,
    index: usize,
) -> std::result::Result<Value, Fields::Error> {
    fields
        .next_element()?
        .ok_or_else(|| de::Error::invalid_length(index, &"a compact queue"))
}

impl<'de, Row: CompactRow> Deserialize<'de> for DecodedQueue<Row> {
    fn deserialize<QueueDeserializer: Deserializer<'de>>(
        deserializer: QueueDeserializer,
    ) -> std::result::Result<Self, QueueDeserializer::Error> {
        deserializer.deserialize_struct("CompactQueue", QUEUE_FIELDS, QueueVisitor(PhantomData))
    }
}

impl<'de, Row: CompactRow> Visitor<'de> for QueueVisitor<Row> {
    type Value = DecodedQueue<Row>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a compact queue")
    }

    fn visit_seq<Fields: SeqAccess<'de>>(
        self,
        mut fields: Fields,
    ) -> std::result::Result<DecodedQueue<Row>, Fields::Error> {
        let tables = DecodedTables::new(next_field(&mut fields, 0)?, next_field(&mut fields, 1)?);
        let items = fields
            .next_element_seed(TrackRowsSeed::<Row> {
                tables: &tables,
                row: PhantomData,
            })?
            .ok_or_else(|| de::Error::invalid_length(TRACKS_FIELD_INDEX, &self))?;
        let snapshot = QueueSnapshot {
            items,
            shuffled: next_field(&mut fields, 3)?,
            current_uid: next_field(&mut fields, 4)?,
            repeat: next_field(&mut fields, 5)?,
            next_uid: next_field(&mut fields, 6)?,
        };
        let position_ms = next_field(&mut fields, 7)?;
        Ok(DecodedQueue {
            stored: StoredQueue {
                snapshot,
                position_ms,
            },
            row: PhantomData,
        })
    }
}

impl<'de, Row: CompactRow> DeserializeSeed<'de> for TrackRowsSeed<'_, Row> {
    type Value = Vec<QueueItem>;

    fn deserialize<RowsDeserializer: Deserializer<'de>>(
        self,
        deserializer: RowsDeserializer,
    ) -> std::result::Result<Vec<QueueItem>, RowsDeserializer::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de, Row: CompactRow> Visitor<'de> for TrackRowsSeed<'_, Row> {
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
        while let Some(row) = rows.next_element::<Row>()? {
            items.push(self.tables.item_of(row.into()).map_err(de::Error::custom)?);
        }
        Ok(items)
    }
}
