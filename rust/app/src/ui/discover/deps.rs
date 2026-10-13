use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use feishin_storage::settings::YouTubeSettings;
use feishin_youtube::music::SystemLocale;

use crate::discover::{DiscoverSource, LibraryMatcher};
use crate::ui::Context;

#[derive(Clone)]
pub struct DiscoverDeps {
    pub ctx: Context,
    pub source: Arc<dyn DiscoverSource>,
    pub matcher: LibraryMatcher,
    pub system: Arc<SystemLocale>,
    pub latest_play: Arc<AtomicU64>,
}

impl DiscoverDeps {
    pub fn new(ctx: &Context, source: Arc<dyn DiscoverSource>) -> Self {
        Self {
            ctx: ctx.clone(),
            source,
            matcher: LibraryMatcher::default(),
            system: Arc::new(SystemLocale::from_env()),
            latest_play: Arc::default(),
        }
    }

    pub fn settings(&self) -> YouTubeSettings {
        self.ctx
            .store
            .load_settings()
            .map(|settings| settings.youtube)
            .unwrap_or_else(|error| {
                tracing::warn!("reading the YouTube settings failed: {error:#}");
                YouTubeSettings::default()
            })
    }
}
