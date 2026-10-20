use tokio::sync::{Mutex, MutexGuard};

use super::backend::Backend;

static NAVIDROME_IN_USE: Mutex<()> = Mutex::const_new(());
static NAVIDROME_054_IN_USE: Mutex<()> = Mutex::const_new(());
static NAVIDROME_056_IN_USE: Mutex<()> = Mutex::const_new(());
static GONIC_IN_USE: Mutex<()> = Mutex::const_new(());
static JELLYFIN_IN_USE: Mutex<()> = Mutex::const_new(());
static JELLYFIN_10_IN_USE: Mutex<()> = Mutex::const_new(());

impl Backend {
    pub async fn exclusive(self) -> MutexGuard<'static, ()> {
        let server_in_use = match self {
            Self::Navidrome | Self::NavidromeSubsonic => &NAVIDROME_IN_USE,
            Self::Navidrome054 => &NAVIDROME_054_IN_USE,
            Self::Navidrome056 => &NAVIDROME_056_IN_USE,
            Self::Gonic => &GONIC_IN_USE,
            Self::Jellyfin => &JELLYFIN_IN_USE,
            Self::Jellyfin10 => &JELLYFIN_10_IN_USE,
        };
        server_in_use.lock().await
    }
}
