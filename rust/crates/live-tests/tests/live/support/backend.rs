use feishin_api::{Credential, MusicServer, Result};
use feishin_api_jellyfin::JellyfinServer;
use feishin_api_navidrome::NavidromeServer;
use feishin_api_subsonic::SubsonicServer;
use feishin_core::model::{ServerConfig, ServerType};
use tokio::sync::OnceCell;

static NAVIDROME_CREDENTIAL: OnceCell<Credential> = OnceCell::const_new();
static GONIC_CREDENTIAL: OnceCell<Credential> = OnceCell::const_new();
static JELLYFIN_CREDENTIAL: OnceCell<Credential> = OnceCell::const_new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Navidrome,
    Gonic,
    Jellyfin,
}

pub struct LiveTarget {
    pub url: String,
    pub username: String,
    pub password: String,
}

fn required_variable(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| {
        panic!("{name} is not set; start the servers with rust/scripts/live-servers.sh up")
    })
}

impl Backend {
    fn environment_prefix(self) -> &'static str {
        match self {
            Self::Navidrome => "FEISHIN_LIVE_NAVIDROME",
            Self::Gonic => "FEISHIN_LIVE_GONIC",
            Self::Jellyfin => "FEISHIN_LIVE_JELLYFIN",
        }
    }

    fn server_type(self) -> ServerType {
        match self {
            Self::Navidrome => ServerType::Navidrome,
            Self::Gonic => ServerType::Subsonic,
            Self::Jellyfin => ServerType::Jellyfin,
        }
    }

    fn cached_credential(self) -> &'static OnceCell<Credential> {
        match self {
            Self::Navidrome => &NAVIDROME_CREDENTIAL,
            Self::Gonic => &GONIC_CREDENTIAL,
            Self::Jellyfin => &JELLYFIN_CREDENTIAL,
        }
    }

    pub fn sorts_any_list(self) -> bool {
        self != Self::Gonic
    }

    pub fn filters_songs_by_year(self) -> bool {
        self != Self::Gonic
    }

    pub fn sorts_by_counts(self) -> bool {
        self != Self::Jellyfin
    }

    pub fn supports_ratings(self) -> bool {
        self != Self::Jellyfin
    }

    pub fn supports_play_queue(self) -> bool {
        self != Self::Jellyfin
    }

    pub fn target(self) -> LiveTarget {
        let prefix = self.environment_prefix();
        LiveTarget {
            url: required_variable(&format!("{prefix}_URL")),
            username: required_variable(&format!("{prefix}_USER")),
            password: required_variable(&format!("{prefix}_PASSWORD")),
        }
    }

    pub fn config(self) -> ServerConfig {
        self.config_for_device(&format!("live-{self:?}").to_lowercase())
    }

    pub fn login_test_config(self) -> ServerConfig {
        self.config_for_device(&format!("live-{self:?}-login").to_lowercase())
    }

    fn config_for_device(self, id: &str) -> ServerConfig {
        let target = self.target();
        ServerConfig {
            id: id.to_owned(),
            name: format!("{self:?}"),
            server_type: self.server_type(),
            url: target.url,
            username: target.username,
        }
    }

    pub async fn login_with(self, password: &str) -> Result<(Box<dyn MusicServer>, Credential)> {
        self.login_as(self.login_test_config(), password).await
    }

    async fn login_as(
        self,
        config: ServerConfig,
        password: &str,
    ) -> Result<(Box<dyn MusicServer>, Credential)> {
        match self {
            Self::Navidrome => boxed(NavidromeServer::login(config, password).await),
            Self::Gonic => boxed(SubsonicServer::login(config, password).await),
            Self::Jellyfin => boxed(JellyfinServer::login(config, password).await),
        }
    }

    pub async fn connect(self) -> Box<dyn MusicServer> {
        let credential = self
            .cached_credential()
            .get_or_init(|| async {
                let password = self.target().password;
                let login = self.login_as(self.config(), &password).await;
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
        match self {
            Self::Navidrome => Ok(Box::new(NavidromeServer::with_credential(
                config, credential,
            )?)),
            Self::Gonic => Ok(Box::new(SubsonicServer::with_credential(
                config, credential,
            )?)),
            Self::Jellyfin => Ok(Box::new(JellyfinServer::with_credential(
                config, credential,
            )?)),
        }
    }
}

fn boxed<Server: MusicServer + 'static>(
    login: Result<(Server, Credential)>,
) -> Result<(Box<dyn MusicServer>, Credential)> {
    login.map(|(server, credential)| (Box::new(server) as Box<dyn MusicServer>, credential))
}
