use async_trait::async_trait;
use feishin_storage::settings::YouTubeSettings;
use feishin_youtube::music::{
    CollectionPage, MUSIC_API_BASE, MusicAuth, MusicClient, MusicError, MusicSong, SystemLocale,
    private_runtime_dir,
};

use super::feed::{DiscoverFeed, FeedNotice, FeedRequest, music_locale};
use super::sign_in::SignIn;
use super::source::DiscoverSource;

pub struct YouTubeMusicSource {
    system: SystemLocale,
    base_url: String,
    sign_in: SignIn,
}

impl Default for YouTubeMusicSource {
    fn default() -> Self {
        Self::new(
            SystemLocale::from_env(),
            MUSIC_API_BASE,
            SignIn::new(private_runtime_dir()),
        )
    }
}

impl YouTubeMusicSource {
    pub fn new(system: SystemLocale, base_url: &str, sign_in: SignIn) -> Self {
        Self {
            system,
            base_url: base_url.to_owned(),
            sign_in,
        }
    }

    fn client(
        &self,
        settings: &YouTubeSettings,
        auth: Option<MusicAuth>,
    ) -> Result<MusicClient, MusicError> {
        Ok(MusicClient::new(music_locale(settings, &self.system))?
            .with_base_url(self.base_url.as_str())
            .with_auth(auth))
    }

    async fn signed_in_client(
        &self,
        settings: &YouTubeSettings,
    ) -> Result<MusicClient, MusicError> {
        let auth = self.sign_in.current(settings);
        self.client(settings, auth)
    }

    async fn anonymous_home(
        &self,
        settings: &YouTubeSettings,
        notice: FeedNotice,
    ) -> Result<DiscoverFeed, MusicError> {
        let shelves = self.client(settings, None)?.home().await?;
        Ok(DiscoverFeed::of_shelves(shelves, notice))
    }

    async fn for_you(&self, settings: &YouTubeSettings) -> Result<DiscoverFeed, MusicError> {
        let Some(browser) = settings.cookies_from_browser else {
            return self.anonymous_home(settings, FeedNotice::SignInHint).await;
        };
        let auth = match self.sign_in.auth_for(settings).await {
            Ok(auth) => auth,
            Err(error) => {
                tracing::warn!("YouTube Music sign-in failed: {error}");
                let notice = FeedNotice::SignInFailed(sign_in_reason(&error));
                return self.anonymous_home(settings, notice).await;
            }
        };
        match self.client(settings, Some(auth))?.home().await {
            Ok(shelves) => Ok(DiscoverFeed::of_shelves(
                shelves,
                FeedNotice::Personalized(browser),
            )),
            Err(error) => {
                tracing::warn!("the personalized YouTube Music home failed: {error}");
                if error.is_rejected_sign_in() {
                    self.sign_in.forget();
                }
                let notice = FeedNotice::SignInFailed(sign_in_reason(&error));
                self.anonymous_home(settings, notice).await
            }
        }
    }
}

fn sign_in_reason(error: &MusicError) -> String {
    match error {
        MusicError::SignIn(reason) => reason.clone(),
        other => other.to_string(),
    }
}

#[async_trait]
impl DiscoverSource for YouTubeMusicSource {
    async fn feed(
        &self,
        request: FeedRequest,
        settings: YouTubeSettings,
    ) -> Result<DiscoverFeed, MusicError> {
        let client = || self.client(&settings, None);
        match request {
            FeedRequest::ForYou => self.for_you(&settings).await,
            FeedRequest::NewReleases => Ok(DiscoverFeed::of_shelves(
                client()?.new_releases().await?,
                FeedNotice::None,
            )),
            FeedRequest::Charts { country } => Ok(DiscoverFeed::of_shelves(
                client()?.charts(&country).await?,
                FeedNotice::None,
            )),
            FeedRequest::Moods => Ok(DiscoverFeed {
                moods: client()?.mood_categories().await?,
                ..DiscoverFeed::default()
            }),
            FeedRequest::Mood { params } => Ok(DiscoverFeed::of_shelves(
                client()?.mood(&params).await?,
                FeedNotice::None,
            )),
        }
    }

    async fn collection(
        &self,
        browse_id: String,
        settings: YouTubeSettings,
    ) -> Result<CollectionPage, MusicError> {
        let signed_in = self.signed_in_client(&settings).await?;
        match signed_in.collection(&browse_id).await {
            Err(error) if error.is_rejected_sign_in() && signed_in.is_signed_in() => {
                self.sign_in.forget();
                self.client(&settings, None)?.collection(&browse_id).await
            }
            other => other,
        }
    }

    async fn search(
        &self,
        query: String,
        settings: YouTubeSettings,
    ) -> Result<Vec<MusicSong>, MusicError> {
        let signed_in = self.signed_in_client(&settings).await?;
        match signed_in.search_songs(&query).await {
            Err(error) if error.is_rejected_sign_in() && signed_in.is_signed_in() => {
                self.sign_in.forget();
                self.client(&settings, None)?.search_songs(&query).await
            }
            other => other,
        }
    }
}
