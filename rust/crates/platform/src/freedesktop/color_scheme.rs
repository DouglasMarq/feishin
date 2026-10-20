use futures_util::StreamExt;
use mpris_server::zbus::{self, Connection, Proxy, proxy::Builder, zvariant::OwnedValue};

use crate::{PlatformError, Result, SystemColorScheme};

const PORTAL_DESTINATION: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const SETTINGS_INTERFACE: &str = "org.freedesktop.portal.Settings";
const APPEARANCE_NAMESPACE: &str = "org.freedesktop.appearance";
const COLOR_SCHEME_KEY: &str = "color-scheme";
const UNKNOWN_METHOD: &str = "org.freedesktop.DBus.Error.UnknownMethod";
const PORTAL_PREFERS_DARK: u32 = 1;
const PORTAL_PREFERS_LIGHT: u32 = 2;

fn scheme_from_portal_value(value: &OwnedValue) -> SystemColorScheme {
    match value.downcast_ref::<u32>() {
        Ok(PORTAL_PREFERS_DARK) => SystemColorScheme::Dark,
        Ok(PORTAL_PREFERS_LIGHT) => SystemColorScheme::Light,
        _ => SystemColorScheme::NoPreference,
    }
}

fn desktop_error(error: zbus::Error) -> PlatformError {
    PlatformError::DesktopSettings(Box::new(error))
}

async fn settings_proxy(connection: &Connection) -> zbus::Result<Proxy<'static>> {
    Builder::new(connection)
        .destination(PORTAL_DESTINATION)?
        .path(PORTAL_PATH)?
        .interface(SETTINGS_INTERFACE)?
        .build()
        .await
}

async fn read_color_scheme(proxy: &Proxy<'_>) -> zbus::Result<OwnedValue> {
    let arguments = (APPEARANCE_NAMESPACE, COLOR_SCHEME_KEY);
    match proxy.call("ReadOne", &arguments).await {
        Err(zbus::Error::MethodError(name, ..)) if name.as_str() == UNKNOWN_METHOD => {
            proxy.call("Read", &arguments).await
        }
        result => result,
    }
}

fn color_scheme_change(message: &zbus::Message) -> Option<SystemColorScheme> {
    let (namespace, key, value) = message
        .body()
        .deserialize::<(String, String, OwnedValue)>()
        .inspect_err(|error| tracing::debug!("ignoring a malformed portal setting change: {error}"))
        .ok()?;
    (namespace == APPEARANCE_NAMESPACE && key == COLOR_SCHEME_KEY)
        .then(|| scheme_from_portal_value(&value))
}

pub async fn current_color_scheme() -> Result<SystemColorScheme> {
    let connection = Connection::session().await.map_err(desktop_error)?;
    let proxy = settings_proxy(&connection).await.map_err(desktop_error)?;
    let current = read_color_scheme(&proxy).await.map_err(desktop_error)?;
    Ok(scheme_from_portal_value(&current))
}

pub async fn watch_color_scheme(mut on_change: impl FnMut(SystemColorScheme)) -> Result<()> {
    let connection = Connection::session().await.map_err(desktop_error)?;
    let proxy = settings_proxy(&connection).await.map_err(desktop_error)?;
    let mut changes = proxy
        .receive_signal("SettingChanged")
        .await
        .map_err(desktop_error)?;
    let current = read_color_scheme(&proxy).await.map_err(desktop_error)?;
    on_change(scheme_from_portal_value(&current));
    while let Some(message) = changes.next().await {
        if let Some(scheme) = color_scheme_change(&message) {
            on_change(scheme);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use mpris_server::zbus::zvariant::Value;

    use super::*;

    fn portal_value(raw: u32) -> OwnedValue {
        OwnedValue::try_from(Value::from(raw)).unwrap()
    }

    #[test]
    fn portal_values_map_to_schemes() {
        assert_eq!(
            scheme_from_portal_value(&portal_value(1)),
            SystemColorScheme::Dark
        );
        assert_eq!(
            scheme_from_portal_value(&portal_value(2)),
            SystemColorScheme::Light
        );
        assert_eq!(
            scheme_from_portal_value(&portal_value(0)),
            SystemColorScheme::NoPreference
        );
    }

    #[test]
    fn values_of_another_type_mean_no_preference() {
        let text = OwnedValue::try_from(Value::from("dark")).unwrap();
        assert_eq!(
            scheme_from_portal_value(&text),
            SystemColorScheme::NoPreference
        );
    }

    #[test]
    fn deprecated_read_replies_are_unwrapped() {
        let nested = OwnedValue::try_from(Value::new(Value::from(1_u32))).unwrap();
        assert_eq!(scheme_from_portal_value(&nested), SystemColorScheme::Dark);
    }
}
