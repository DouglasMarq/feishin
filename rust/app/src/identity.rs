use feishin_platform::MediaIdentity;

pub const APP_NAME: &str = "Cadence";
pub const XDG_APP_ID: &str = "org.jeffvli.feishin.native";

pub const MEDIA_IDENTITY: MediaIdentity<'static> = MediaIdentity {
    name: APP_NAME,
    desktop_entry: XDG_APP_ID,
};

pub fn announce_xdg_app_id() {
    if let Err(error) = slint::set_xdg_app_id(XDG_APP_ID) {
        tracing::warn!("setting the window app id failed: {error}");
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;

    fn packaging_file(extension: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../packaging")
            .join(format!("{XDG_APP_ID}.{extension}"))
    }

    #[test]
    fn desktop_entry_is_named_after_the_app_id() {
        let desktop_entry = std::fs::read_to_string(packaging_file("desktop")).unwrap();
        assert!(desktop_entry.contains(&format!("Icon={XDG_APP_ID}\n")));
        assert!(desktop_entry.contains(&format!("StartupWMClass={XDG_APP_ID}\n")));
        assert!(desktop_entry.contains(&format!("Name={APP_NAME}\n")));
    }

    #[test]
    fn metainfo_declares_the_app_id_and_desktop_entry() {
        let metainfo = std::fs::read_to_string(packaging_file("metainfo.xml")).unwrap();
        assert!(metainfo.contains(&format!("<id>{XDG_APP_ID}</id>")));
        assert!(metainfo.contains(&format!(
            "<launchable type=\"desktop-id\">{XDG_APP_ID}.desktop</launchable>"
        )));
    }
}
