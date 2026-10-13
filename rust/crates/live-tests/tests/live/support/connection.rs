use feishin_api::{Credential, MusicServer, Result};
use feishin_api_jellyfin::JellyfinServer;
use feishin_api_navidrome::NavidromeServer;
use feishin_api_subsonic::SubsonicServer;
use feishin_core::model::{ServerConfig, ServerType};
use tokio::sync::OnceCell;

use super::backend::Backend;

static NAVIDROME_CREDENTIAL: OnceCell<Credential> = OnceCell::const_new();
static NAVIDROME_054_CREDENTIAL: OnceCell<Credential> = OnceCell::const_new();
static NAVIDROME_056_CREDENTIAL: OnceCell<Credential> = OnceCell::const_new();
static GONIC_CREDENTIAL: OnceCell<Credential> = OnceCell::const_new();
static JELLYFIN_CREDENTIAL: OnceCell<Credential> = OnceCell::const_new();
static JELLYFIN_10_CREDENTIAL: OnceCell<Credential> = OnceCell::const_new();

type LoggedIn = (Box<dyn MusicServer>, Credential);

fn cached_credential(backend: Backend) -> &'static OnceCell<Credential> {
    match backend {
        Backend::Navidrome | Backend::NavidromeSubsonic => &NAVIDROME_CREDENTIAL,
        Backend::Navidrome054 => &NAVIDROME_054_CREDENTIAL,
        Backend::Navidrome056 => &NAVIDROME_056_CREDENTIAL,
        Backend::Gonic => &GONIC_CREDENTIAL,
        Backend::Jellyfin => &JELLYFIN_CREDENTIAL,
        Backend::Jellyfin10 => &JELLYFIN_10_CREDENTIAL,
    }
}

fn without_session(credential: &Credential) -> Credential {
    match credential {
        Credential::Navidrome { subsonic, .. } => Credential::Navidrome {
            subsonic: subsonic.clone(),
            nd_token: None,
        },
        other => other.clone(),
    }
}

fn boxed<Server: MusicServer + 'static>(login: Result<(Server, Credential)>) -> Result<LoggedIn> {
    login.map(|(server, credential)| (Box::new(server) as Box<dyn MusicServer>, credential))
}

async fn login_as(backend: Backend, config: ServerConfig, password: &str) -> Result<LoggedIn> {
    match backend.server_type() {
        ServerType::Navidrome => boxed(NavidromeServer::login(config, password).await),
        ServerType::Subsonic => boxed(SubsonicServer::login(config, password).await),
        ServerType::Jellyfin => boxed(JellyfinServer::login(config, password).await),
    }
}

impl Backend {
    pub async fn login_with(self, password: &str) -> Result<LoggedIn> {
        login_as(self, self.login_test_config(), password).await
    }

    pub async fn connect(self) -> Box<dyn MusicServer> {
        let credential = cached_credential(self)
            .get_or_init(|| async {
                let password = self.target().password;
                let login = login_as(self, self.config(), &password).await;
                login.expect("login").1
            })
            .await;
        self.reconnect(self.config(), credential)
            .expect("reconnect")
    }

    pub fn reconnect(
        self,
        config: ServerConfig,
        credential: &Credential,
    ) -> Result<Box<dyn MusicServer>> {
        let credential = match self {
            Self::NavidromeSubsonic => without_session(credential),
            _ => credential.clone(),
        };
        match self.server_type() {
            ServerType::Navidrome => Ok(Box::new(NavidromeServer::with_credential(
                config,
                &credential,
            )?)),
            ServerType::Subsonic => Ok(Box::new(SubsonicServer::with_credential(
                config,
                &credential,
            )?)),
            ServerType::Jellyfin => Ok(Box::new(JellyfinServer::with_credential(
                config,
                &credential,
            )?)),
        }
    }
}
