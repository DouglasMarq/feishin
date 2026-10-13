use std::sync::{Arc, RwLock};

use feishin_api::MusicServer;

#[derive(Clone, Default)]
pub struct Session(Arc<RwLock<Option<Arc<dyn MusicServer>>>>);

impl Session {
    pub fn get(&self) -> Option<Arc<dyn MusicServer>> {
        self.0.read().ok().and_then(|server| server.clone())
    }

    pub fn set(&self, server: Option<Arc<dyn MusicServer>>) {
        if let Ok(mut current) = self.0.write() {
            *current = server;
        }
    }
}
