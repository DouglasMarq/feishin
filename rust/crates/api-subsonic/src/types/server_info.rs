use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Ping {
    pub open_subsonic: Option<bool>,
    pub server_version: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Extension {
    pub name: String,
    pub versions: Vec<u32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct OpenSubsonicExtensions {
    pub open_subsonic_extensions: Option<Vec<Extension>>,
}
