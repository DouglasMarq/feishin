use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackSource {
    #[default]
    Server,
    YouTube(Arc<YouTubeVideo>),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct YouTubeVideo {
    pub video_id: Box<str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<Box<str>>,
}

impl TrackSource {
    pub fn is_server(&self) -> bool {
        matches!(self, Self::Server)
    }

    pub fn youtube(&self) -> Option<&YouTubeVideo> {
        match self {
            Self::Server => None,
            Self::YouTube(video) => Some(video),
        }
    }
}
