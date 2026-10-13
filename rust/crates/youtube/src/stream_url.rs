use std::time::{SystemTime, UNIX_EPOCH};

const EXPIRY_PARAMETER: &str = "expire";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamUrl {
    pub url: String,
    pub expires_at: Option<u64>,
}

impl StreamUrl {
    pub fn new(url: String) -> Self {
        let expires_at = expiry_of(&url);
        Self { url, expires_at }
    }
}

fn expiry_of(url: &str) -> Option<u64> {
    let (_, query) = url.split_once('?')?;
    let query = query.split('#').next().unwrap_or_default();
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(name, _)| *name == EXPIRY_PARAMETER)
        .and_then(|(_, value)| value.parse().ok())
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since_epoch| since_epoch.as_secs())
        .unwrap_or_default()
}
