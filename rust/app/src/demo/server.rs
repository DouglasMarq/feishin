use std::sync::Mutex;

use feishin_api::Credential;
use feishin_core::model::{ServerConfig, ServerPlayQueue};

use super::playlists::DemoPlaylists;
use super::scrobbles::ScrobbleCall;
use super::song_order::CachedSongOrder;

pub struct FakeServer {
    pub(super) config: ServerConfig,
    pub(super) song_count: usize,
    pub(super) order_cache: Mutex<Option<CachedSongOrder>>,
    pub(super) scrobbles: Mutex<Vec<ScrobbleCall>>,
    pub(super) playlists: Mutex<DemoPlaylists>,
    pub(super) play_queue: Mutex<Option<ServerPlayQueue>>,
    pub(super) queue_saves: Mutex<usize>,
    pub(super) credential: Mutex<Option<Credential>>,
}

impl FakeServer {
    pub fn with_config(config: ServerConfig, song_count: usize) -> Self {
        Self {
            config,
            song_count,
            order_cache: Mutex::default(),
            scrobbles: Mutex::default(),
            playlists: Mutex::new(DemoPlaylists::generated(song_count)),
            play_queue: Mutex::default(),
            queue_saves: Mutex::default(),
            credential: Mutex::default(),
        }
    }

    pub(super) fn current_credential(&self) -> Option<Credential> {
        self.credential
            .lock()
            .ok()
            .and_then(|credential| credential.clone())
    }
}
