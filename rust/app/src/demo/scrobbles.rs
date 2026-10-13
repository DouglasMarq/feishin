use super::server::FakeServer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrobbleCall {
    pub song_id: String,
    pub submission: bool,
    pub timestamp_ms: Option<i64>,
}

impl FakeServer {
    pub(super) fn record_scrobble(&self, call: ScrobbleCall) {
        match self.scrobbles.lock() {
            Ok(mut calls) => calls.push(call),
            Err(error) => tracing::warn!("demo scrobble log unavailable: {error}"),
        }
    }
}
