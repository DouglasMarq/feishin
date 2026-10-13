use std::sync::Mutex;

use feishin_api::{ApiError, Result};
use feishin_core::model::{ServerType, Share};

use super::server::FakeServer;

const DEMO_SHARE_HOST: &str = "https://share.feishin.invalid";

#[derive(Default)]
pub(super) struct DemoShares {
    created: Mutex<Vec<Vec<String>>>,
}

impl FakeServer {
    pub(super) fn demo_can_share(&self) -> bool {
        self.config.server_type != ServerType::Jellyfin
    }

    pub(super) fn create_demo_share(
        &self,
        item_ids: &[String],
        description: Option<&str>,
    ) -> Result<Share> {
        if !self.demo_can_share() {
            return Err(ApiError::Unsupported("public share links"));
        }
        let mut created = self.shares.created.lock().map_err(|_| ApiError::Server {
            code: 0,
            message: "demo share list is unavailable".into(),
        })?;
        created.push(item_ids.to_vec());
        let id = format!("demo-share-{}", created.len());
        Ok(Share {
            url: format!("{DEMO_SHARE_HOST}/{id}"),
            id,
            description: description.map(str::to_owned),
            expires_at: None,
        })
    }

    #[cfg(test)]
    pub fn shared_items(&self) -> Vec<Vec<String>> {
        self.shares
            .created
            .lock()
            .map(|created| created.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::DemoLibrary;

    #[test]
    fn demo_shares_get_numbered_links() {
        let server = FakeServer::new(100);
        let first = server.create_demo_share(&["album-1".into()], None).unwrap();
        let second = server
            .create_demo_share(&["song-1".into(), "song-2".into()], Some("Mix"))
            .unwrap();
        assert_eq!(first.url, "https://share.feishin.invalid/demo-share-1");
        assert_eq!(second.id, "demo-share-2");
        assert_eq!(second.description.as_deref(), Some("Mix"));
        assert_eq!(server.shared_items().len(), 2);
    }

    #[test]
    fn a_demo_jellyfin_server_cannot_share() {
        let config = feishin_core::model::ServerConfig {
            server_type: ServerType::Jellyfin,
            ..FakeServer::config_for_demo()
        };
        let server = FakeServer::with_config(config, DemoLibrary::with_songs(10));
        assert!(!server.demo_can_share());
        assert!(matches!(
            server.create_demo_share(&["song-1".into()], None),
            Err(ApiError::Unsupported(_))
        ));
    }
}
