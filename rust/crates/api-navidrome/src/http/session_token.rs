use std::sync::PoisonError;

use reqwest::header::HeaderMap;

use crate::NavidromeServer;

pub(crate) const SESSION_TOKEN_HEADER: &str = "x-nd-authorization";

impl NavidromeServer {
    pub(crate) fn current_session_token(&self) -> Option<String> {
        self.session_token
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub(crate) fn has_session_token(&self) -> bool {
        self.current_session_token().is_some()
    }

    pub(crate) fn replace_session_token(&self, token: Option<String>) {
        *self
            .session_token
            .write()
            .unwrap_or_else(PoisonError::into_inner) = token;
    }

    pub(crate) fn capture_refreshed_token(&self, headers: &HeaderMap) {
        if let Some(token) = refreshed_token(headers) {
            self.replace_session_token(Some(token.to_owned()));
        }
    }
}

fn refreshed_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(SESSION_TOKEN_HEADER)
        .and_then(|header| header.to_str().ok())
        .map(str::trim)
        .map(|header| header.strip_prefix("Bearer ").unwrap_or(header).trim())
        .filter(|token| !token.is_empty())
}
