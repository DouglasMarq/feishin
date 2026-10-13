use std::sync::Mutex;

use feishin_api::Credential;
use feishin_core::model::ServerConfig;

use super::artist_albums::ArtistAlbums;
use super::library::DemoLibrary;
use super::play_queue::DemoPlayQueue;
use super::playlists::DemoPlaylists;
use super::scanning::DemoScan;
use super::scrobbles::ScrobbleCall;
use super::song_order::CachedSongOrder;

pub struct FakeServer {
    pub(super) config: ServerConfig,
    pub(super) library: DemoLibrary,
    pub(super) artist_albums: ArtistAlbums,
    pub(super) order_cache: Mutex<Option<CachedSongOrder>>,
    pub(super) scrobbles: Mutex<Vec<ScrobbleCall>>,
    pub(super) playlists: Mutex<DemoPlaylists>,
    pub(super) play_queue: Mutex<Option<DemoPlayQueue>>,
    pub(super) queue_saves: Mutex<usize>,
    pub(super) credential: Mutex<Option<Credential>>,
    pub(super) scan: DemoScan,
}

impl FakeServer {
    pub fn with_config(config: ServerConfig, library: DemoLibrary) -> Self {
        Self {
            config,
            library,
            artist_albums: ArtistAlbums::index(library),
            order_cache: Mutex::default(),
            scrobbles: Mutex::default(),
            playlists: Mutex::new(DemoPlaylists::generated(library.song_count())),
            play_queue: Mutex::default(),
            queue_saves: Mutex::default(),
            credential: Mutex::default(),
            scan: DemoScan::default(),
        }
    }

    pub(super) fn current_credential(&self) -> Option<Credential> {
        self.credential
            .lock()
            .ok()
            .and_then(|credential| credential.clone())
    }
}
