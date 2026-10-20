use serde::Serialize;

use super::locale::MusicLocale;

const CLIENT_NAME: &str = "WEB_REMIX";
const CLIENT_VERSION: &str = "1.20260901.01.00";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClientInfo<'request> {
    client_name: &'static str,
    client_version: &'static str,
    hl: &'request str,
    gl: &'request str,
}

#[derive(Debug, Serialize)]
pub(crate) struct RequestContext<'request> {
    client: ClientInfo<'request>,
}

impl<'request> RequestContext<'request> {
    pub(crate) fn new(locale: &'request MusicLocale) -> Self {
        Self {
            client: ClientInfo {
                client_name: CLIENT_NAME,
                client_version: CLIENT_VERSION,
                hl: &locale.language,
                gl: &locale.country,
            },
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FormData<'request> {
    selected_values: [&'request str; 1],
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BrowseRequest<'request> {
    context: RequestContext<'request>,
    browse_id: &'request str,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<&'request str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    form_data: Option<FormData<'request>>,
}

impl<'request> BrowseRequest<'request> {
    pub(crate) fn new(locale: &'request MusicLocale, browse_id: &'request str) -> Self {
        Self {
            context: RequestContext::new(locale),
            browse_id,
            params: None,
            form_data: None,
        }
    }

    pub(crate) fn with_params(self, params: &'request str) -> Self {
        Self {
            params: Some(params),
            ..self
        }
    }

    pub(crate) fn with_selected_value(self, value: &'request str) -> Self {
        Self {
            form_data: Some(FormData {
                selected_values: [value],
            }),
            ..self
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct SearchRequest<'request> {
    context: RequestContext<'request>,
    query: &'request str,
    params: &'static str,
}

impl<'request> SearchRequest<'request> {
    pub(crate) fn new(
        locale: &'request MusicLocale,
        query: &'request str,
        params: &'static str,
    ) -> Self {
        Self {
            context: RequestContext::new(locale),
            query,
            params,
        }
    }
}

const RADIO_PLAYLIST_PREFIX: &str = "RDAMVM";
const NORMAL_AUTOMIX: &str = "AUTOMIX_SETTING_NORMAL";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NextRequest<'request> {
    context: RequestContext<'request>,
    video_id: &'request str,
    playlist_id: String,
    is_audio_only: bool,
    #[serde(rename = "enablePersistentPlaylistPanel")]
    is_persistent_playlist_panel_enabled: bool,
    tuner_setting_value: &'static str,
}

impl<'request> NextRequest<'request> {
    pub(crate) fn radio(locale: &'request MusicLocale, video_id: &'request str) -> Self {
        Self {
            context: RequestContext::new(locale),
            video_id,
            playlist_id: format!("{RADIO_PLAYLIST_PREFIX}{video_id}"),
            is_audio_only: true,
            is_persistent_playlist_panel_enabled: true,
            tuner_setting_value: NORMAL_AUTOMIX,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn locale() -> MusicLocale {
        MusicLocale {
            language: "pt".into(),
            country: "BR".into(),
        }
    }

    #[test]
    fn browse_bodies_match_the_web_client() {
        let locale = locale();
        let body = serde_json::to_value(
            BrowseRequest::new(&locale, "FEmusic_charts").with_selected_value("US"),
        )
        .unwrap();
        assert_eq!(
            body,
            json!({
                "context": {"client": {"clientName": "WEB_REMIX", "clientVersion": "1.20260901.01.00", "hl": "pt", "gl": "BR"}},
                "browseId": "FEmusic_charts",
                "formData": {"selectedValues": ["US"]}
            })
        );
        let mood = serde_json::to_value(BrowseRequest::new(&locale, "X").with_params("p")).unwrap();
        assert_eq!(mood["params"], "p");
        assert!(mood.get("formData").is_none());
    }

    #[test]
    fn radio_bodies_ask_for_the_song_radio_playlist() {
        let locale = locale();
        let body = serde_json::to_value(NextRequest::radio(&locale, "abc")).unwrap();
        assert_eq!(
            body,
            json!({
                "context": {"client": {"clientName": "WEB_REMIX", "clientVersion": "1.20260901.01.00", "hl": "pt", "gl": "BR"}},
                "videoId": "abc",
                "playlistId": "RDAMVMabc",
                "isAudioOnly": true,
                "enablePersistentPlaylistPanel": true,
                "tunerSettingValue": "AUTOMIX_SETTING_NORMAL"
            })
        );
    }
}
