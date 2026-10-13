use feishin_core::model::{ItemKind, ServerConfig, ServerType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Navidrome,
    Navidrome054,
    Navidrome056,
    NavidromeSubsonic,
    Gonic,
    Jellyfin,
    Jellyfin10,
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
            Self::Navidrome | Self::NavidromeSubsonic => "FEISHIN_LIVE_NAVIDROME",
            Self::Navidrome054 => "FEISHIN_LIVE_NAVIDROME_054",
            Self::Navidrome056 => "FEISHIN_LIVE_NAVIDROME_056",
            Self::Gonic => "FEISHIN_LIVE_GONIC",
            Self::Jellyfin => "FEISHIN_LIVE_JELLYFIN",
            Self::Jellyfin10 => "FEISHIN_LIVE_JELLYFIN_10",
        }
    }

    pub fn server_type(self) -> ServerType {
        match self {
            Self::Gonic => ServerType::Subsonic,
            Self::Jellyfin | Self::Jellyfin10 => ServerType::Jellyfin,
            _ => ServerType::Navidrome,
        }
    }

    fn is_jellyfin(self) -> bool {
        self.server_type() == ServerType::Jellyfin
    }

    pub fn has_native_navidrome_session(self) -> bool {
        matches!(
            self,
            Self::Navidrome | Self::Navidrome054 | Self::Navidrome056
        )
    }

    pub fn sorts_any_list(self) -> bool {
        !matches!(self, Self::Gonic | Self::NavidromeSubsonic)
    }

    pub fn lists_years_newest_first(self) -> bool {
        self == Self::Gonic
    }

    pub fn filters_songs_by_year(self) -> bool {
        self != Self::Gonic
    }

    pub fn sorts_by_counts(self) -> bool {
        !self.is_jellyfin()
    }

    pub fn clears_play_queue(self) -> bool {
        self != Self::Gonic
    }

    pub fn has_an_entry_id_per_playlist_position(self) -> bool {
        !self.is_jellyfin()
    }

    pub fn keeps_repeated_playlist_songs(self) -> bool {
        self != Self::Jellyfin10
    }

    pub fn counts_albums_in_artist_lists(self) -> bool {
        self != Self::Jellyfin10
    }

    pub fn supports_ratings(self) -> bool {
        !self.is_jellyfin()
    }

    pub fn supports_play_queue(self) -> bool {
        !self.is_jellyfin()
    }

    pub fn missing_id(self, kind: ItemKind) -> &'static str {
        match (self.server_type(), kind) {
            (ServerType::Jellyfin, _) => "ffffffffffffffffffffffffffffffff",
            (ServerType::Navidrome, _) => "feishin-missing-item",
            (ServerType::Subsonic, ItemKind::Song) => "tr-99999",
            (ServerType::Subsonic, ItemKind::Album) => "al-99999",
            (ServerType::Subsonic, ItemKind::Artist) => "ar-99999",
            (ServerType::Subsonic, ItemKind::Playlist) => "pl-MS9taXNzaW5nLm0zdQ==",
        }
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
}
