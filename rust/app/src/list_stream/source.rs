use std::sync::Arc;

use async_trait::async_trait;
use feishin_api::MusicServer;
use feishin_core::model::{Page, Paging, Song, SongListQuery};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistId(pub String);

#[derive(Debug, Clone, PartialEq)]
pub enum SongSource {
    Library(SongListQuery),
    Playlist(PlaylistId),
}

#[async_trait]
pub trait PageFetcher: Send + Sync {
    async fn fetch(&self, paging: Paging) -> feishin_api::Result<Page<Song>>;
}

pub struct ServerPages {
    pub server: Arc<dyn MusicServer>,
    pub source: SongSource,
}

#[async_trait]
impl PageFetcher for ServerPages {
    async fn fetch(&self, paging: Paging) -> feishin_api::Result<Page<Song>> {
        crate::library::song_page(self.server.as_ref(), &self.source, paging).await
    }
}
