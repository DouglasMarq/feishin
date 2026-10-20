use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{AUTHORIZATION, COOKIE, ORIGIN, USER_AGENT};
use serde::Serialize;
use serde_json::Value;

use super::auth::MusicAuth;
use super::error::MusicError;
use super::locale::MusicLocale;
use super::model::{
    CollectionPage, MoodCategory, MusicShelf, MusicSong, collection_kind, is_browse_id,
};
use super::raw;
use super::request::{BrowseRequest, NextRequest, SearchRequest};
use crate::{is_video_id, unix_now};

pub const MUSIC_API_BASE: &str = "https://music.youtube.com/youtubei/v1";
const MUSIC_ORIGIN: &str = "https://music.youtube.com";
const BROWSER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0";
const X_ORIGIN: &str = "X-Origin";
const X_AUTH_USER: &str = "X-Goog-AuthUser";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const SONGS_FILTER: &str = "EgWKAQIIAWoMEA4QChADEAQQCRAF";
const HOME: &str = "FEmusic_home";
const NEW_RELEASES: &str = "FEmusic_new_releases_albums";
const CHARTS: &str = "FEmusic_charts";
const EXPLORE: &str = "FEmusic_explore";

#[derive(Debug, Clone)]
pub struct MusicClient {
    http: reqwest::Client,
    base_url: String,
    locale: MusicLocale,
    auth: Option<MusicAuth>,
}

impl MusicClient {
    pub fn new(locale: MusicLocale) -> Result<Self, MusicError> {
        let http = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(network_error)?;
        Ok(Self {
            http,
            base_url: MUSIC_API_BASE.to_owned(),
            locale,
            auth: None,
        })
    }

    pub fn with_base_url(self, base_url: impl Into<String>) -> Self {
        let base_url: String = base_url.into();
        Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            ..self
        }
    }

    pub fn with_auth(self, auth: Option<MusicAuth>) -> Self {
        Self { auth, ..self }
    }

    pub fn is_signed_in(&self) -> bool {
        self.auth.is_some()
    }

    pub async fn home(&self) -> Result<Vec<MusicShelf>, MusicError> {
        self.shelves(BrowseRequest::new(&self.locale, HOME)).await
    }

    pub async fn new_releases(&self) -> Result<Vec<MusicShelf>, MusicError> {
        self.shelves(BrowseRequest::new(&self.locale, NEW_RELEASES))
            .await
    }

    pub async fn charts(&self, country_code: &str) -> Result<Vec<MusicShelf>, MusicError> {
        let request = BrowseRequest::new(&self.locale, CHARTS).with_selected_value(country_code);
        self.shelves(request).await
    }

    pub async fn mood_categories(&self) -> Result<Vec<MoodCategory>, MusicError> {
        let response = self
            .browse(&BrowseRequest::new(&self.locale, EXPLORE))
            .await?;
        Ok(raw::mood_categories(&response))
    }

    pub async fn mood(&self, params: &str) -> Result<Vec<MusicShelf>, MusicError> {
        let request =
            BrowseRequest::new(&self.locale, raw::MOODS_CATEGORY_BROWSE_ID).with_params(params);
        self.shelves(request).await
    }

    pub async fn collection(&self, browse_id: &str) -> Result<CollectionPage, MusicError> {
        if !is_browse_id(browse_id) {
            return Err(MusicError::InvalidId(browse_id.to_owned()));
        }
        let response = self
            .browse(&BrowseRequest::new(&self.locale, browse_id))
            .await?;
        raw::collection_page(&response, collection_kind(browse_id))
            .ok_or_else(|| MusicError::Decode(format!("no tracks in {browse_id}")))
    }

    pub async fn search_songs(&self, query: &str) -> Result<Vec<MusicSong>, MusicError> {
        let request = SearchRequest::new(&self.locale, query.trim(), SONGS_FILTER);
        let response = self.post("search", &request).await?;
        match raw::has_search_sections(&response) {
            true => Ok(raw::search_songs(&response)),
            false => Ok(Vec::new()),
        }
    }

    pub async fn radio(&self, video_id: &str) -> Result<Vec<MusicSong>, MusicError> {
        if !is_video_id(video_id) {
            return Err(MusicError::InvalidId(video_id.to_owned()));
        }
        let response = self
            .post("next", &NextRequest::radio(&self.locale, video_id))
            .await?;
        match raw::has_watch_panel(&response) {
            true => Ok(raw::watch_songs(&response)),
            false => Err(MusicError::Decode(format!("no radio for {video_id}"))),
        }
    }

    async fn shelves(&self, request: BrowseRequest<'_>) -> Result<Vec<MusicShelf>, MusicError> {
        let response = self.browse(&request).await?;
        if !raw::has_sections(&response) {
            return Err(MusicError::Decode("the page has no shelves".to_owned()));
        }
        Ok(raw::browse_shelves(&response))
    }

    async fn browse(&self, request: &BrowseRequest<'_>) -> Result<Value, MusicError> {
        self.post("browse", request).await
    }

    async fn post(&self, endpoint: &str, body: &impl Serialize) -> Result<Value, MusicError> {
        let url = format!("{}/{endpoint}?prettyPrint=false", self.base_url);
        let request = self
            .http
            .post(url)
            .header(ORIGIN, MUSIC_ORIGIN)
            .header(USER_AGENT, BROWSER_AGENT)
            .json(body);
        let response = self.signed(request).send().await.map_err(network_error)?;
        let status = response.status();
        if !status.is_success() {
            return Err(MusicError::Status(status.as_u16()));
        }
        let bytes = response.bytes().await.map_err(network_error)?;
        serde_json::from_slice(&bytes).map_err(|error| MusicError::Malformed(Arc::new(error)))
    }

    fn signed(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let Some(auth) = &self.auth else {
            return request;
        };
        request
            .header(COOKIE, auth.cookie_header())
            .header(AUTHORIZATION, auth.authorization(unix_now()))
            .header(X_ORIGIN, MUSIC_ORIGIN)
            .header(X_AUTH_USER, "0")
    }
}

fn network_error(error: reqwest::Error) -> MusicError {
    MusicError::Network(Arc::new(error.without_url()))
}
