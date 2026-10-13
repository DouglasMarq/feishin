use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AuthenticateByName<'a> {
    #[serde(rename = "Pw")]
    pub password: &'a str,
    pub username: &'a str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct NewPlaylist<'a> {
    pub media_type: &'a str,
    pub name: &'a str,
    pub user_id: &'a str,
}

#[derive(Debug, Serialize)]
pub struct EmptyBody {}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlaybackReport<'a> {
    pub item_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_paused: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_ticks: Option<u64>,
}
