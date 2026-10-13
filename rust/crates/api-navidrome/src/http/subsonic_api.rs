use feishin_api::Result;
use reqwest::header::CONTENT_TYPE;
use serde::de::DeserializeOwned;
use url::form_urlencoded;

use crate::NavidromeServer;
use crate::features::{Features, OpenSubsonicExtension};
use crate::http::errors::{http_status_error, network_error};
use crate::http::subsonic_envelope::decode_subsonic_envelope;

const API_VERSION: &str = "1.16.1";
const CLIENT_NAME: &str = "feishin";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResponseFormat {
    Json,
    ServerDefault,
}

pub(crate) fn query_string(params: &[(&str, String)]) -> String {
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    for (key, value) in params {
        serializer.append_pair(key, value);
    }
    serializer.finish()
}

impl NavidromeServer {
    pub(crate) fn subsonic_url(
        &self,
        endpoint: &str,
        params: &[(&str, String)],
        format: ResponseFormat,
    ) -> String {
        format!(
            "{}/rest/{endpoint}.view?{}",
            self.base_url,
            self.subsonic_query(params, format)
        )
    }

    pub(crate) async fn subsonic<Decoded: DeserializeOwned>(
        &self,
        endpoint: &str,
        params: &[(&str, String)],
    ) -> Result<Decoded> {
        let response = self
            .http_client
            .get(self.subsonic_url(endpoint, params, ResponseFormat::Json))
            .send()
            .await
            .map_err(network_error)?;
        decode_subsonic_response(response).await
    }

    pub(crate) async fn subsonic_large<Decoded: DeserializeOwned>(
        &self,
        endpoint: &str,
        params: &[(&str, String)],
        features: &Features,
    ) -> Result<Decoded> {
        if !features.has_extension(OpenSubsonicExtension::FormPost) {
            return self.subsonic(endpoint, params).await;
        }
        let response = self
            .http_client
            .post(format!("{}/rest/{endpoint}.view", self.base_url))
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(self.subsonic_query(params, ResponseFormat::Json))
            .send()
            .await
            .map_err(network_error)?;
        decode_subsonic_response(response).await
    }

    fn subsonic_query(&self, params: &[(&str, String)], format: ResponseFormat) -> String {
        let mut common = vec![("v", API_VERSION.to_owned()), ("c", CLIENT_NAME.to_owned())];
        if format == ResponseFormat::Json {
            common.push(("f", "json".to_owned()));
        }
        common.extend(params.iter().map(|(key, value)| (*key, value.clone())));
        let query = query_string(&common);
        match self.subsonic_auth.as_str() {
            "" => query,
            auth => format!("{auth}&{query}"),
        }
    }
}

async fn decode_subsonic_response<Decoded: DeserializeOwned>(
    response: reqwest::Response,
) -> Result<Decoded> {
    let status = response.status();
    let body = response.bytes().await.map_err(network_error)?;
    if !status.is_success() {
        return Err(http_status_error(status, &body));
    }
    decode_subsonic_envelope(&body)
}
