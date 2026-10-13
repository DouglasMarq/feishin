use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) enum SmartPlaylistRules {}

#[derive(Debug, Serialize)]
pub(crate) struct CreatePlaylistRequest<'name> {
    pub name: &'name str,
    pub rules: Option<SmartPlaylistRules>,
}

#[derive(Debug, Serialize)]
pub(crate) struct TrackIdsRequest<'ids> {
    pub ids: &'ids [String],
}

#[derive(Debug, Serialize)]
pub(crate) struct SaveQueueRequest<'ids> {
    pub ids: &'ids [String],
    pub position: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<usize>,
}
