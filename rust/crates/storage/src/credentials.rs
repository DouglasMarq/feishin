use std::sync::{Arc, OnceLock};

use keyring_core::{Entry, Error};

use crate::{Result, StorageError};

pub const SERVICE: &str = "feishin-rs";

pub fn set(server_id: &str, credential: &str) -> Result<()> {
    run_outside_async_runtime(|| {
        entry(server_id)?
            .set_password(credential)
            .map_err(keyring_error)
    })
}

pub fn get(server_id: &str) -> Result<Option<String>> {
    run_outside_async_runtime(|| match entry(server_id)?.get_password() {
        Ok(credential) => Ok(Some(credential)),
        Err(Error::NoEntry) => Ok(None),
        Err(error) => Err(keyring_error(error)),
    })
}

pub fn delete(server_id: &str) -> Result<()> {
    run_outside_async_runtime(|| match entry(server_id)?.delete_credential() {
        Ok(()) | Err(Error::NoEntry) => Ok(()),
        Err(error) => Err(keyring_error(error)),
    })
}

fn run_outside_async_runtime<T: Send>(
    keyring_call: impl FnOnce() -> Result<T> + Send,
) -> Result<T> {
    std::thread::scope(|scope| {
        scope
            .spawn(keyring_call)
            .join()
            .unwrap_or_else(|_| Err(StorageError::Keyring("keyring thread panicked".into())))
    })
}

pub fn use_in_memory_store() -> Result<()> {
    let store = keyring_core::mock::Store::new().map_err(keyring_error)?;
    keyring_core::set_default_store(store);
    Ok(())
}

fn entry(server_id: &str) -> Result<Entry> {
    ensure_store()?;
    Entry::new(SERVICE, server_id).map_err(keyring_error)
}

fn ensure_store() -> Result<()> {
    if keyring_core::get_default_store().is_some() {
        return Ok(());
    }
    static PLATFORM_STORE_INSTALLATION: OnceLock<std::result::Result<(), String>> = OnceLock::new();
    PLATFORM_STORE_INSTALLATION
        .get_or_init(install_platform_store)
        .clone()
        .map_err(StorageError::Keyring)
}

fn install_platform_store() -> std::result::Result<(), String> {
    let store = platform_store().map_err(|error| error.to_string())?;
    if keyring_core::get_default_store().is_none() {
        keyring_core::set_default_store(store);
    }
    Ok(())
}

#[cfg(all(
    unix,
    not(any(target_os = "macos", target_os = "ios", target_os = "android"))
))]
fn platform_store() -> keyring_core::Result<Arc<keyring_core::CredentialStore>> {
    let store: Arc<keyring_core::CredentialStore> =
        zbus_secret_service_keyring_store::Store::new()?;
    Ok(store)
}

#[cfg(not(all(
    unix,
    not(any(target_os = "macos", target_os = "ios", target_os = "android"))
)))]
fn platform_store() -> keyring_core::Result<Arc<keyring_core::CredentialStore>> {
    Err(Error::NotSupportedByStore(
        "no keyring backend for this platform yet".into(),
    ))
}

fn keyring_error(error: Error) -> StorageError {
    StorageError::Keyring(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn use_mock_keyring() {
        static MOCK_KEYRING_INSTALLED: std::sync::Once = std::sync::Once::new();
        MOCK_KEYRING_INSTALLED.call_once(|| use_in_memory_store().unwrap());
    }

    #[test]
    fn set_get_delete() {
        use_mock_keyring();
        assert_eq!(get("server-a").unwrap(), None);
        set(
            "server-a",
            "{\"type\":\"subsonic\",\"subsonic\":\"u=a&p=b\"}",
        )
        .unwrap();
        assert_eq!(
            get("server-a").unwrap().as_deref(),
            Some("{\"type\":\"subsonic\",\"subsonic\":\"u=a&p=b\"}")
        );
        set("server-a", "replaced").unwrap();
        assert_eq!(get("server-a").unwrap().as_deref(), Some("replaced"));
        delete("server-a").unwrap();
        assert_eq!(get("server-a").unwrap(), None);
    }

    #[test]
    fn delete_missing_is_ok() {
        use_mock_keyring();
        delete("never-saved").unwrap();
    }

    #[test]
    fn entries_are_isolated_by_server_id() {
        use_mock_keyring();
        set("server-1", "one").unwrap();
        set("server-2", "two").unwrap();
        assert_eq!(get("server-1").unwrap().as_deref(), Some("one"));
        assert_eq!(get("server-2").unwrap().as_deref(), Some("two"));
        delete("server-1").unwrap();
        assert_eq!(get("server-1").unwrap(), None);
        assert_eq!(get("server-2").unwrap().as_deref(), Some("two"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn works_inside_a_tokio_runtime() {
        use_mock_keyring();
        set("server-rt", "inside").unwrap();
        assert_eq!(get("server-rt").unwrap().as_deref(), Some("inside"));
        delete("server-rt").unwrap();
    }

    #[test]
    fn credential_json_round_trips_through_keyring() {
        use_mock_keyring();
        let credential = feishin_api::Credential::Navidrome {
            subsonic: "u=me&s=salt&t=token".into(),
            nd_token: Some("jwt".into()),
        };
        set("server-nd", &credential.to_json()).unwrap();
        let stored = get("server-nd").unwrap().unwrap();
        assert_eq!(
            feishin_api::Credential::from_json(&stored).unwrap(),
            credential
        );
    }
}
