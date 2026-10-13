use std::path::PathBuf;

use feishin_storage::settings::YouTubeSettings;
use feishin_youtube::CookieBrowser;
use feishin_youtube::music::{MusicAuth, MusicError, sign_in_with_browser};
use tokio::sync::Mutex;

use crate::youtube::yt_dlp_for;

#[derive(Default)]
pub struct SignIn {
    private_dir: Option<PathBuf>,
    signed_in: Mutex<Option<(CookieBrowser, MusicAuth)>>,
}

impl SignIn {
    pub fn new(private_dir: Option<PathBuf>) -> Self {
        Self {
            private_dir,
            signed_in: Mutex::default(),
        }
    }

    pub async fn auth_for(&self, settings: &YouTubeSettings) -> Result<MusicAuth, MusicError> {
        let browser = settings
            .cookies_from_browser
            .ok_or_else(|| MusicError::SignIn("no browser is selected".to_owned()))?;
        let mut signed_in = self.signed_in.lock().await;
        if let Some((known_browser, auth)) = signed_in.as_ref()
            && *known_browser == browser
        {
            return Ok(auth.clone());
        }
        let private_dir = self.private_dir.as_deref().ok_or_else(|| {
            MusicError::SignIn(
                "XDG_RUNTIME_DIR is not set, so there is no private place for the cookies"
                    .to_owned(),
            )
        })?;
        let auth = sign_in_with_browser(&yt_dlp_for(settings), private_dir).await?;
        *signed_in = Some((browser, auth.clone()));
        Ok(auth)
    }

    pub async fn current(&self, settings: &YouTubeSettings) -> Option<MusicAuth> {
        let browser = settings.cookies_from_browser?;
        let signed_in = self.signed_in.lock().await;
        signed_in
            .as_ref()
            .filter(|(known_browser, _)| *known_browser == browser)
            .map(|(_, auth)| auth.clone())
    }

    pub async fn forget(&self) {
        *self.signed_in.lock().await = None;
    }
}
