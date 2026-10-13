use std::path::PathBuf;
use std::sync::Mutex;

use feishin_storage::settings::YouTubeSettings;
use feishin_youtube::CookieBrowser;
use feishin_youtube::music::{MusicAuth, MusicError, sign_in_with_browser};

use crate::youtube::yt_dlp_for;

type BrowserSession = (CookieBrowser, MusicAuth);

#[derive(Default)]
pub struct SignIn {
    private_dir: Option<PathBuf>,
    signed_in: Mutex<Option<BrowserSession>>,
    exporting: tokio::sync::Mutex<()>,
}

impl SignIn {
    pub fn new(private_dir: Option<PathBuf>) -> Self {
        Self {
            private_dir,
            ..Self::default()
        }
    }

    pub async fn auth_for(&self, settings: &YouTubeSettings) -> Result<MusicAuth, MusicError> {
        let browser = settings
            .cookies_from_browser
            .ok_or_else(|| MusicError::SignIn("no browser is selected".to_owned()))?;
        if let Some(auth) = self.current(settings) {
            return Ok(auth);
        }
        let _exporting = self.exporting.lock().await;
        if let Some(auth) = self.current(settings) {
            return Ok(auth);
        }
        let private_dir = self.private_dir.as_deref().ok_or_else(|| {
            MusicError::SignIn(
                "XDG_RUNTIME_DIR is not set, so there is no private place for the cookies"
                    .to_owned(),
            )
        })?;
        let auth = sign_in_with_browser(&yt_dlp_for(settings), private_dir).await?;
        self.remember(Some((browser, auth.clone())));
        Ok(auth)
    }

    pub fn current(&self, settings: &YouTubeSettings) -> Option<MusicAuth> {
        let browser = settings.cookies_from_browser?;
        let signed_in = self
            .signed_in
            .lock()
            .inspect_err(|error| tracing::warn!("the YouTube Music sign-in is unreadable: {error}"))
            .ok()?;
        signed_in
            .as_ref()
            .filter(|(known_browser, _)| *known_browser == browser)
            .map(|(_, auth)| auth.clone())
    }

    pub fn forget(&self) {
        self.remember(None);
    }

    fn remember(&self, session: Option<BrowserSession>) {
        match self.signed_in.lock() {
            Ok(mut signed_in) => *signed_in = session,
            Err(error) => tracing::warn!("could not keep the YouTube Music sign-in: {error}"),
        }
    }
}
