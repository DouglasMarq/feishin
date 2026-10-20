use serde::Serialize;

#[derive(Serialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackEvent {
    Pause,
    Unpause,
}

impl PlaybackEvent {
    pub fn for_paused_state(is_paused: bool) -> Self {
        match is_paused {
            true => Self::Pause,
            false => Self::Unpause,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlaybackReport<'a> {
    pub item_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_name: Option<PlaybackEvent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_paused: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_ticks: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playback_event_serializes_as_lowercase_event_name() {
        let paused = PlaybackEvent::for_paused_state(true);
        let resumed = PlaybackEvent::for_paused_state(false);
        assert_eq!(serde_json::to_string(&paused).unwrap(), r#""pause""#);
        assert_eq!(serde_json::to_string(&resumed).unwrap(), r#""unpause""#);
    }
}
