mod credential;
mod envelope;
mod params;
mod payload;

use std::time::Duration;

use feishin_api::{ApiError, Result};
use reqwest::{RequestBuilder, Response};

pub use credential::{
    encode_uri_component, password_credential, token_credential,
    validate_token_or_password_credential,
};
pub use params::Params;
pub use payload::Payload;

use envelope::unwrap_subsonic_response;

pub const API_VERSION: &str = "1.13.0";
pub const CLIENT_NAME: &str = "Feishin";

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestEncoding {
    QueryString,
    FormPost,
}

pub struct Client {
    http: reqwest::Client,
    base: String,
    auth: String,
}

fn network_error(error: reqwest::Error) -> ApiError {
    ApiError::Network(error.to_string())
}

fn total_count_header(response: &Response) -> Option<usize> {
    response
        .headers()
        .get("x-total-count")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.trim().parse().ok())
}

impl Client {
    pub fn new(base: &str, auth: String) -> Result<Self> {
        let http = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(network_error)?;
        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_owned(),
            auth,
        })
    }

    pub fn auth(&self) -> &str {
        &self.auth
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    fn query(&self, params: &Params) -> String {
        let mut query = format!("{}&v={API_VERSION}&c={CLIENT_NAME}&f=json", self.auth);
        params.append_encoded_to(&mut query);
        query
    }

    fn build_request(
        &self,
        url: String,
        query: String,
        encoding: RequestEncoding,
    ) -> RequestBuilder {
        match encoding {
            RequestEncoding::FormPost => self
                .http
                .post(url)
                .header(
                    reqwest::header::CONTENT_TYPE,
                    "application/x-www-form-urlencoded",
                )
                .body(query),
            RequestEncoding::QueryString => self.http.get(format!("{url}?{query}")),
        }
    }

    pub async fn call(
        &self,
        endpoint: &str,
        params: &Params,
        encoding: RequestEncoding,
    ) -> Result<Payload> {
        let url = format!("{}/rest/{endpoint}", self.base);
        let request = self.build_request(url, self.query(params), encoding);
        let response = request.send().await.map_err(network_error)?;
        let http_status = response.status().as_u16();
        let total_count = total_count_header(&response);
        let text = response.text().await.map_err(network_error)?;
        let envelope = unwrap_subsonic_response(http_status, &text)?;
        Ok(Payload::new(endpoint, envelope, total_count))
    }
}
