//! Imports servers saved by the Electron Feishin (zustand `store_authentication` in Chromium
//! localStorage under `~/.config/feishin/Local Storage/leveldb`).

use std::fmt;
use std::path::{Path, PathBuf};

use feishin_api::Credential;
use feishin_core::model::{ServerConfig, ServerType};
use rusty_leveldb::{DB, LdbIterator, Options};
use serde::Deserialize;
use serde::de::{MapAccess, Visitor};

use crate::{Result, StorageError, xdg_base};

pub const STORE_KEY: &str = "store_authentication";
const PREFERRED_ORIGIN: &str = "file://";

#[derive(Debug, Clone, PartialEq)]
pub struct ImportedServer {
    pub config: ServerConfig,
    /// Ready for `feishin_storage::credentials::set(id, &credential.to_json())`, when Feishin saved one.
    pub credential: Option<Credential>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct FeishinImport {
    pub servers: Vec<ImportedServer>,
    /// The server selected in Feishin, when it is among `servers`.
    pub current_server_id: Option<String>,
}

/// `$XDG_CONFIG_HOME/feishin`, falling back to `~/.config/feishin`.
pub fn default_config_dir() -> Result<PathBuf> {
    Ok(xdg_base(&|name| std::env::var_os(name), "XDG_CONFIG_HOME", ".config")?.join("feishin"))
}

/// `feishin_config_dir` is usually `~/.config/feishin`.
pub fn import_servers(feishin_config_dir: &Path) -> Result<Vec<ImportedServer>> {
    Ok(import(feishin_config_dir)?.servers)
}

/// Reads a copy of Feishin's localStorage. Empty when Feishin was never run or saved no servers.
pub fn import(feishin_config_dir: &Path) -> Result<FeishinImport> {
    let leveldb_dir = feishin_config_dir.join("Local Storage").join("leveldb");
    if !leveldb_dir.is_dir() {
        return Ok(FeishinImport::default());
    }
    match read_store_value(&leveldb_dir)? {
        Some(json) => parse_auth_store(&json),
        None => Ok(FeishinImport::default()),
    }
}

/// Parses the persisted zustand JSON (`{"state":{"serverList":{..},"currentServer":..},"version":2}`).
pub fn parse_auth_store(json: &str) -> Result<FeishinImport> {
    let persisted: Persisted = serde_json::from_str(json)
        .map_err(|e| StorageError::Invalid(format!("{STORE_KEY}: {e}")))?;
    let servers: Vec<ImportedServer> = persisted
        .state
        .server_list
        .0
        .into_iter()
        .filter_map(
            |(key, value)| match serde_json::from_value::<RawServer>(value) {
                Ok(raw) => map_server(&key, raw),
                Err(e) => {
                    tracing::warn!("skipping Feishin server {key}: {e}");
                    None
                }
            },
        )
        .collect();
    let current_server_id = persisted
        .state
        .current_server
        .and_then(|current| current.id)
        .filter(|id| servers.iter().any(|s| &s.config.id == id));
    Ok(FeishinImport {
        servers,
        current_server_id,
    })
}

fn read_store_value(leveldb_dir: &Path) -> Result<Option<String>> {
    let copy = tempfile::tempdir()?;
    for entry in std::fs::read_dir(leveldb_dir)? {
        let entry = entry?;
        if entry.file_name() == "LOCK" || !entry.file_type()?.is_file() {
            continue;
        }
        std::fs::copy(entry.path(), copy.path().join(entry.file_name()))?;
    }

    let options = Options {
        create_if_missing: false,
        ..Options::default()
    };
    let mut db = DB::open(copy.path(), options).map_err(leveldb_error)?;
    let mut iter = db.new_iter().map_err(leveldb_error)?;
    let mut found: Option<(String, Vec<u8>)> = None;
    while iter.advance() {
        let Some((key, value)) = iter.current() else {
            continue;
        };
        let Some((origin, script_key)) = split_local_storage_key(&key) else {
            continue;
        };
        if decode_chromium_string(script_key).as_deref() != Some(STORE_KEY) {
            continue;
        }
        let better = match &found {
            None => true,
            Some((current, _)) => current != PREFERRED_ORIGIN && origin == PREFERRED_ORIGIN,
        };
        if better {
            found = Some((origin, value.to_vec()));
        }
    }
    drop(iter);
    drop(db);

    found
        .map(|(_, value)| {
            decode_chromium_string(&value)
                .ok_or_else(|| StorageError::Invalid(format!("{STORE_KEY}: bad value encoding")))
        })
        .transpose()
}

/// Splits `_<origin>\0<encoded key>` into the origin and the still-encoded script key.
fn split_local_storage_key(key: &[u8]) -> Option<(String, &[u8])> {
    let rest = key.strip_prefix(b"_")?;
    let nul = rest.iter().position(|&b| b == 0)?;
    let origin = String::from_utf8_lossy(&rest[..nul]).into_owned();
    Some((origin, &rest[nul + 1..]))
}

/// Chromium localStorage string encoding: a leading `0` means UTF-16LE, `1` means Latin-1.
fn decode_chromium_string(bytes: &[u8]) -> Option<String> {
    let (&format, data) = bytes.split_first()?;
    match format {
        0 => {
            let (pairs, rest) = data.as_chunks::<2>();
            if !rest.is_empty() {
                return None;
            }
            let units: Vec<u16> = pairs.iter().map(|&pair| u16::from_le_bytes(pair)).collect();
            Some(String::from_utf16_lossy(&units))
        }
        1 => Some(data.iter().map(|&b| char::from(b)).collect()),
        _ => None,
    }
}

fn leveldb_error(e: rusty_leveldb::Status) -> StorageError {
    StorageError::Invalid(format!("Feishin localStorage: {e}"))
}

fn map_server(key: &str, raw: RawServer) -> Option<ImportedServer> {
    let server_type = match raw.server_type.as_deref()?.to_ascii_lowercase().as_str() {
        "navidrome" => ServerType::Navidrome,
        "subsonic" => ServerType::Subsonic,
        "jellyfin" => ServerType::Jellyfin,
        other => {
            tracing::warn!("skipping Feishin server {key}: unsupported type {other}");
            return None;
        }
    };
    let remote_url = raw.remote_url.filter(|url| !url.trim().is_empty());
    let url = match (raw.prefer_remote_url.unwrap_or(false), remote_url) {
        (true, Some(remote)) => remote,
        _ => raw.url.filter(|url| !url.trim().is_empty())?,
    };
    let url = normalize_url(&url);
    let id = non_empty(raw.id).unwrap_or_else(|| key.to_string());
    let name = non_empty(raw.name).unwrap_or_else(|| url.clone());

    let token = non_empty(raw.credential);
    let credential = match server_type {
        ServerType::Navidrome => token.map(|subsonic| Credential::Navidrome {
            subsonic,
            nd_token: non_empty(raw.nd_credential),
        }),
        ServerType::Subsonic => token.map(|subsonic| Credential::Subsonic { subsonic }),
        ServerType::Jellyfin => token
            .zip(non_empty(raw.user_id))
            .map(|(access_token, user_id)| Credential::Jellyfin {
                access_token,
                user_id,
            }),
    };

    Some(ImportedServer {
        config: ServerConfig {
            id,
            name,
            server_type,
            url,
            username: raw.username.unwrap_or_default(),
        },
        credential,
    })
}

/// Mirrors `normalizeServerUrl` in `src/renderer/utils/normalize-server-url.ts`.
fn normalize_url(url: &str) -> String {
    let trimmed = url.trim();
    trimmed.strip_suffix('/').unwrap_or(trimmed).to_string()
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.is_empty())
}

#[derive(Deserialize)]
struct Persisted {
    #[serde(default)]
    state: AuthState,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct AuthState {
    server_list: OrderedEntries,
    current_server: Option<CurrentServer>,
}

#[derive(Deserialize)]
struct CurrentServer {
    id: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct RawServer {
    credential: Option<String>,
    id: Option<String>,
    name: Option<String>,
    nd_credential: Option<String>,
    prefer_remote_url: Option<bool>,
    remote_url: Option<String>,
    #[serde(rename = "type")]
    server_type: Option<String>,
    url: Option<String>,
    user_id: Option<String>,
    username: Option<String>,
}

/// A JSON object kept in document order (Feishin lists servers in insertion order).
#[derive(Default)]
struct OrderedEntries(Vec<(String, serde_json::Value)>);

impl<'de> Deserialize<'de> for OrderedEntries {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct EntriesVisitor;

        impl<'de> Visitor<'de> for EntriesVisitor {
            type Value = OrderedEntries;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an object or null")
            }

            fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Self::Value, E> {
                Ok(OrderedEntries::default())
            }

            fn visit_map<A: MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<Self::Value, A::Error> {
                let mut entries = Vec::new();
                while let Some(entry) = map.next_entry::<String, serde_json::Value>()? {
                    entries.push(entry);
                }
                Ok(OrderedEntries(entries))
            }
        }

        deserializer.deserialize_any(EntriesVisitor)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn latin1(s: &str) -> Vec<u8> {
        let mut out = vec![1u8];
        out.extend(s.chars().map(|c| u8::try_from(u32::from(c)).unwrap()));
        out
    }

    fn utf16(s: &str) -> Vec<u8> {
        let mut out = vec![0u8];
        out.extend(s.encode_utf16().flat_map(u16::to_le_bytes));
        out
    }

    fn storage_key(origin: &str, key: &str) -> Vec<u8> {
        let mut out = format!("_{origin}\0").into_bytes();
        out.extend(latin1(key));
        out
    }

    fn feishin_dir_with(entries: &[(Vec<u8>, Vec<u8>)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let ldb = dir.path().join("Local Storage").join("leveldb");
        std::fs::create_dir_all(&ldb).unwrap();
        let mut db = DB::open(&ldb, Options::default()).unwrap();
        db.put(b"VERSION", b"1").unwrap();
        db.put(b"META:file://", b"\x08\x01").unwrap();
        for (k, v) in entries {
            db.put(k, v).unwrap();
        }
        db.flush().unwrap();
        db.close().unwrap();
        dir
    }

    fn auth_json() -> String {
        r#"{
            "state": {
                "currentServer": { "id": "nd-1", "name": "Navidrome", "credential": "x" },
                "deviceId": "dev",
                "serverList": {
                    "nd-1": {
                        "credential": "u=alice&s=salt&t=token",
                        "features": {},
                        "id": "nd-1",
                        "isAdmin": true,
                        "name": "Navidrome Música",
                        "ndCredential": "jwt.token.value",
                        "savePassword": false,
                        "type": "navidrome",
                        "url": "https://nd.example.com/",
                        "userId": "user-1",
                        "username": "alice",
                        "version": "0.53.0"
                    },
                    "ss-1": {
                        "credential": "u=bob&p=enc:abc",
                        "id": "ss-1",
                        "name": "Subsonic",
                        "preferRemoteUrl": true,
                        "remoteUrl": "https://remote.example.com",
                        "type": "subsonic",
                        "url": "http://192.168.1.2:4040",
                        "userId": null,
                        "username": "bob"
                    },
                    "jf-1": {
                        "credential": "jellyfin-token",
                        "id": "jf-1",
                        "name": "Jellyfin",
                        "type": "jellyfin",
                        "url": "http://jf.local:8096",
                        "userId": "jf-user",
                        "username": "carol"
                    },
                    "nd-2": {
                        "credential": "",
                        "id": "nd-2",
                        "name": "Logged out",
                        "ndCredential": "",
                        "type": "navidrome",
                        "url": "http://nd2.local",
                        "userId": null,
                        "username": "dave"
                    }
                }
            },
            "version": 2
        }"#
        .to_string()
    }

    fn expected() -> Vec<ImportedServer> {
        vec![
            ImportedServer {
                config: ServerConfig {
                    id: "nd-1".into(),
                    name: "Navidrome Música".into(),
                    server_type: ServerType::Navidrome,
                    url: "https://nd.example.com".into(),
                    username: "alice".into(),
                },
                credential: Some(Credential::Navidrome {
                    subsonic: "u=alice&s=salt&t=token".into(),
                    nd_token: Some("jwt.token.value".into()),
                }),
            },
            ImportedServer {
                config: ServerConfig {
                    id: "ss-1".into(),
                    name: "Subsonic".into(),
                    server_type: ServerType::Subsonic,
                    url: "https://remote.example.com".into(),
                    username: "bob".into(),
                },
                credential: Some(Credential::Subsonic {
                    subsonic: "u=bob&p=enc:abc".into(),
                }),
            },
            ImportedServer {
                config: ServerConfig {
                    id: "jf-1".into(),
                    name: "Jellyfin".into(),
                    server_type: ServerType::Jellyfin,
                    url: "http://jf.local:8096".into(),
                    username: "carol".into(),
                },
                credential: Some(Credential::Jellyfin {
                    access_token: "jellyfin-token".into(),
                    user_id: "jf-user".into(),
                }),
            },
            ImportedServer {
                config: ServerConfig {
                    id: "nd-2".into(),
                    name: "Logged out".into(),
                    server_type: ServerType::Navidrome,
                    url: "http://nd2.local".into(),
                    username: "dave".into(),
                },
                credential: None,
            },
        ]
    }

    #[test]
    fn imports_latin1_value_in_document_order() {
        let dir = feishin_dir_with(&[
            (storage_key("file://", "store_settings"), latin1("{}")),
            (storage_key("file://", STORE_KEY), latin1(&auth_json())),
        ]);
        let imported = import(dir.path()).unwrap();
        assert_eq!(imported.servers, expected());
        assert_eq!(imported.current_server_id.as_deref(), Some("nd-1"));
        assert_eq!(import_servers(dir.path()).unwrap(), expected());
    }

    #[test]
    fn imports_utf16_value() {
        let json = auth_json().replace("Música", "音楽");
        let dir = feishin_dir_with(&[(storage_key("file://", STORE_KEY), utf16(&json))]);
        let servers = import_servers(dir.path()).unwrap();
        assert_eq!(servers.len(), 4);
        assert_eq!(servers[0].config.name, "Navidrome 音楽");
    }

    #[test]
    fn utf16_encoded_key_is_recognized() {
        let mut key = b"_file://\0".to_vec();
        key.extend(utf16(STORE_KEY));
        let dir = feishin_dir_with(&[(key, latin1(&auth_json()))]);
        assert_eq!(import_servers(dir.path()).unwrap(), expected());
    }

    #[test]
    fn prefers_file_origin_over_others() {
        let dev = json!({"state": {"serverList": {}}, "version": 2}).to_string();
        let dir = feishin_dir_with(&[
            (storage_key("app://feishin", STORE_KEY), latin1(&dev)),
            (
                storage_key("http://localhost:5173", STORE_KEY),
                latin1(&dev),
            ),
            (storage_key("file://", STORE_KEY), latin1(&auth_json())),
        ]);
        assert_eq!(import_servers(dir.path()).unwrap(), expected());
    }

    #[test]
    fn falls_back_to_other_origins() {
        let dir = feishin_dir_with(&[(
            storage_key("http://localhost:5173", STORE_KEY),
            latin1(&auth_json()),
        )]);
        assert_eq!(import_servers(dir.path()).unwrap(), expected());
    }

    #[test]
    fn works_while_the_database_is_locked_and_leaves_it_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let ldb = dir.path().join("Local Storage").join("leveldb");
        std::fs::create_dir_all(&ldb).unwrap();
        let mut db = DB::open(&ldb, Options::default()).unwrap();
        db.put(&storage_key("file://", STORE_KEY), &latin1(&auth_json()))
            .unwrap();
        db.flush().unwrap();

        let before: Vec<_> = list_dir(&ldb);
        assert_eq!(import_servers(dir.path()).unwrap(), expected());
        assert_eq!(list_dir(&ldb), before);
        drop(db);
    }

    fn list_dir(dir: &Path) -> Vec<(std::ffi::OsString, u64)> {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                (e.file_name(), e.metadata().unwrap().len())
            })
            .collect();
        entries.sort();
        entries
    }

    #[test]
    fn missing_dir_or_key_imports_nothing() {
        let empty = tempfile::tempdir().unwrap();
        assert_eq!(import(empty.path()).unwrap(), FeishinImport::default());
        let dir = feishin_dir_with(&[(storage_key("file://", "store_app"), latin1("{}"))]);
        assert_eq!(import(dir.path()).unwrap(), FeishinImport::default());
    }

    #[test]
    fn bad_json_is_invalid() {
        let dir = feishin_dir_with(&[(storage_key("file://", STORE_KEY), latin1("{oops"))]);
        assert!(matches!(import(dir.path()), Err(StorageError::Invalid(_))));
    }

    #[test]
    fn skips_malformed_and_unknown_servers() {
        let json = json!({
            "state": {
                "currentServer": { "id": "gone" },
                "serverList": {
                    "weird": { "type": "plex", "url": "http://p", "id": "weird", "credential": "x" },
                    "broken": { "type": "navidrome", "url": 42 },
                    "no-url": { "type": "subsonic", "id": "no-url", "credential": "x" },
                    "jf-no-user": {
                        "type": "Jellyfin", "url": "http://jf", "credential": "tok",
                        "userId": null, "username": "u"
                    },
                    "nd-no-nd": { "type": "navidrome", "url": "http://nd", "credential": "u=a&s=b&t=c" }
                }
            }
        })
        .to_string();
        let imported = parse_auth_store(&json).unwrap();
        assert_eq!(imported.current_server_id, None);
        assert_eq!(imported.servers.len(), 2);
        let jf = &imported.servers[0];
        assert_eq!(jf.config.id, "jf-no-user");
        assert_eq!(jf.config.server_type, ServerType::Jellyfin);
        assert_eq!(jf.config.name, "http://jf");
        assert_eq!(jf.credential, None);
        let nd = &imported.servers[1];
        assert_eq!(
            nd.credential,
            Some(Credential::Navidrome {
                subsonic: "u=a&s=b&t=c".into(),
                nd_token: None,
            })
        );
    }

    #[test]
    fn empty_or_null_server_list() {
        assert_eq!(
            parse_auth_store(r#"{"state":{"serverList":null,"currentServer":null}}"#).unwrap(),
            FeishinImport::default()
        );
        assert_eq!(parse_auth_store("{}").unwrap(), FeishinImport::default());
    }

    #[test]
    fn decodes_chromium_strings() {
        assert_eq!(
            decode_chromium_string(&latin1("café")).as_deref(),
            Some("café")
        );
        assert_eq!(
            decode_chromium_string(&utf16("日本")).as_deref(),
            Some("日本")
        );
        assert_eq!(decode_chromium_string(&[0, 1]), None);
        assert_eq!(decode_chromium_string(&[2, b'a']), None);
        assert_eq!(decode_chromium_string(&[]), None);
    }
}
