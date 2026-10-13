use std::fmt;

use serde::Deserialize;
use serde::de::{MapAccess, Visitor};

#[derive(Deserialize)]
pub(super) struct Persisted {
    #[serde(default)]
    pub(super) state: AuthState,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct AuthState {
    pub(super) server_list: OrderedEntries,
    pub(super) current_server: Option<CurrentServer>,
}

#[derive(Deserialize)]
pub(super) struct CurrentServer {
    pub(super) id: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct RawServer {
    pub(super) credential: Option<String>,
    pub(super) id: Option<String>,
    pub(super) name: Option<String>,
    pub(super) nd_credential: Option<String>,
    pub(super) prefer_remote_url: Option<bool>,
    pub(super) remote_url: Option<String>,
    #[serde(rename = "type")]
    pub(super) server_type: Option<String>,
    pub(super) url: Option<String>,
    pub(super) user_id: Option<String>,
    pub(super) username: Option<String>,
}

pub(super) struct RawServerEntry {
    pub(super) key: String,
    pub(super) value: serde_json::Value,
}

#[derive(Default)]
pub(super) struct OrderedEntries(Vec<RawServerEntry>);

impl OrderedEntries {
    pub(super) fn into_entries(self) -> Vec<RawServerEntry> {
        self.0
    }
}

impl<'de> Deserialize<'de> for OrderedEntries {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        deserializer.deserialize_any(OrderedEntriesVisitor)
    }
}

struct OrderedEntriesVisitor;

impl<'de> Visitor<'de> for OrderedEntriesVisitor {
    type Value = OrderedEntries;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an object or null")
    }

    fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Self::Value, E> {
        Ok(OrderedEntries::default())
    }

    fn visit_map<A: MapAccess<'de>>(
        self,
        mut map: A,
    ) -> std::result::Result<Self::Value, A::Error> {
        let mut entries = Vec::new();
        while let Some((key, value)) = map.next_entry::<String, serde_json::Value>()? {
            entries.push(RawServerEntry { key, value });
        }
        Ok(OrderedEntries(entries))
    }
}
