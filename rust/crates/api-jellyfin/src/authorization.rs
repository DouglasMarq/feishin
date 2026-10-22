const CLIENT_NAME: &str = "Cadence";
const DEVICE_NAME: &str = "Desktop Client";
const CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn auth_header(device_id: &str, token: Option<&str>) -> String {
    let client_identity = format!(
        r#"MediaBrowser Client="{CLIENT_NAME}", Device="{DEVICE_NAME}", DeviceId="{device_id}", Version="{CLIENT_VERSION}""#
    );
    match token {
        Some(token) => format!(r#"{client_identity}, Token="{token}""#),
        None => client_identity,
    }
}
