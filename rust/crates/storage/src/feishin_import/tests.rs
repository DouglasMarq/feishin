use std::path::Path;

use feishin_core::model::ServerType;
use rusty_leveldb::{DB, Options};
use serde_json::json;

use super::local_storage::decode_chromium_string;
use super::*;

fn latin1(text: &str) -> Vec<u8> {
    let mut encoded = vec![1u8];
    encoded.extend(
        text.chars()
            .map(|character| u8::try_from(u32::from(character)).unwrap()),
    );
    encoded
}

fn utf16(text: &str) -> Vec<u8> {
    let mut encoded = vec![0u8];
    encoded.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    encoded
}

fn storage_key(origin: &str, key: &str) -> Vec<u8> {
    let mut encoded = format!("_{origin}\0").into_bytes();
    encoded.extend(latin1(key));
    encoded
}

fn feishin_dir_with(entries: &[(Vec<u8>, Vec<u8>)]) -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let leveldb_dir = directory.path().join("Local Storage").join("leveldb");
    std::fs::create_dir_all(&leveldb_dir).unwrap();
    let mut database = DB::open(&leveldb_dir, Options::default()).unwrap();
    database.put(b"VERSION", b"1").unwrap();
    database.put(b"META:file://", b"\x08\x01").unwrap();
    for (key, value) in entries {
        database.put(key, value).unwrap();
    }
    database.flush().unwrap();
    database.close().unwrap();
    directory
}

fn auth_json() -> String {
    include_str!("auth_store_fixture.json").to_string()
}

fn expected() -> Vec<ImportedServer> {
    vec![
        expected_navidrome(),
        expected_subsonic(),
        expected_jellyfin(),
        expected_logged_out_navidrome(),
    ]
}

fn expected_navidrome() -> ImportedServer {
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
    }
}

fn expected_subsonic() -> ImportedServer {
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
    }
}

fn expected_jellyfin() -> ImportedServer {
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
    }
}

fn expected_logged_out_navidrome() -> ImportedServer {
    ImportedServer {
        config: ServerConfig {
            id: "nd-2".into(),
            name: "Logged out".into(),
            server_type: ServerType::Navidrome,
            url: "http://nd2.local".into(),
            username: "dave".into(),
        },
        credential: None,
    }
}

#[test]
fn imports_latin1_value_in_document_order() {
    let directory = feishin_dir_with(&[
        (storage_key("file://", "store_settings"), latin1("{}")),
        (storage_key("file://", STORE_KEY), latin1(&auth_json())),
    ]);
    let imported = import(directory.path()).unwrap();
    assert_eq!(imported.servers, expected());
    assert_eq!(imported.current_server_id.as_deref(), Some("nd-1"));
    assert_eq!(import_servers(directory.path()).unwrap(), expected());
}

#[test]
fn imports_utf16_value() {
    let json = auth_json().replace("Música", "音楽");
    let directory = feishin_dir_with(&[(storage_key("file://", STORE_KEY), utf16(&json))]);
    let servers = import_servers(directory.path()).unwrap();
    assert_eq!(servers.len(), 4);
    assert_eq!(servers[0].config.name, "Navidrome 音楽");
}

#[test]
fn utf16_encoded_key_is_recognized() {
    let mut key = b"_file://\0".to_vec();
    key.extend(utf16(STORE_KEY));
    let directory = feishin_dir_with(&[(key, latin1(&auth_json()))]);
    assert_eq!(import_servers(directory.path()).unwrap(), expected());
}

#[test]
fn prefers_file_origin_over_others() {
    let dev = json!({"state": {"serverList": {}}, "version": 2}).to_string();
    let directory = feishin_dir_with(&[
        (storage_key("app://feishin", STORE_KEY), latin1(&dev)),
        (
            storage_key("http://localhost:5173", STORE_KEY),
            latin1(&dev),
        ),
        (storage_key("file://", STORE_KEY), latin1(&auth_json())),
    ]);
    assert_eq!(import_servers(directory.path()).unwrap(), expected());
}

#[test]
fn falls_back_to_other_origins() {
    let directory = feishin_dir_with(&[(
        storage_key("http://localhost:5173", STORE_KEY),
        latin1(&auth_json()),
    )]);
    assert_eq!(import_servers(directory.path()).unwrap(), expected());
}

#[test]
fn works_while_the_database_is_locked_and_leaves_it_untouched() {
    let directory = tempfile::tempdir().unwrap();
    let leveldb_dir = directory.path().join("Local Storage").join("leveldb");
    std::fs::create_dir_all(&leveldb_dir).unwrap();
    let mut database = DB::open(&leveldb_dir, Options::default()).unwrap();
    database
        .put(&storage_key("file://", STORE_KEY), &latin1(&auth_json()))
        .unwrap();
    database.flush().unwrap();

    let before: Vec<_> = list_dir(&leveldb_dir);
    assert_eq!(import_servers(directory.path()).unwrap(), expected());
    assert_eq!(list_dir(&leveldb_dir), before);
    drop(database);
}

fn list_dir(directory: &Path) -> Vec<(std::ffi::OsString, u64)> {
    let mut entries: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (entry.file_name(), entry.metadata().unwrap().len())
        })
        .collect();
    entries.sort();
    entries
}

#[test]
fn missing_dir_or_key_imports_nothing() {
    let empty = tempfile::tempdir().unwrap();
    assert_eq!(import(empty.path()).unwrap(), FeishinImport::default());
    let directory = feishin_dir_with(&[(storage_key("file://", "store_app"), latin1("{}"))]);
    assert_eq!(import(directory.path()).unwrap(), FeishinImport::default());
}

#[test]
fn bad_json_is_invalid() {
    let directory = feishin_dir_with(&[(storage_key("file://", STORE_KEY), latin1("{oops"))]);
    assert!(matches!(
        import(directory.path()),
        Err(StorageError::Invalid(_))
    ));
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
    let jellyfin = &imported.servers[0];
    assert_eq!(jellyfin.config.id, "jf-no-user");
    assert_eq!(jellyfin.config.server_type, ServerType::Jellyfin);
    assert_eq!(jellyfin.config.name, "http://jf");
    assert_eq!(jellyfin.credential, None);
    let navidrome = &imported.servers[1];
    assert_eq!(
        navidrome.credential,
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
