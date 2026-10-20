use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use feishin_storage::settings::YouTubeSettings;
use feishin_youtube::music::SystemLocale;

use super::send_order::SendOrder;
use crate::discover::{DiscoverSource, LibraryMatcher};
use crate::ui::Context;

#[derive(Clone)]
pub struct DiscoverDeps {
    pub context: Context,
    pub source: Arc<dyn DiscoverSource>,
    pub matcher: LibraryMatcher,
    pub system: Arc<SystemLocale>,
    pub latest_play: Arc<AtomicU64>,
    pub send_order: SendOrder,
}

impl DiscoverDeps {
    pub fn new(context: &Context, source: Arc<dyn DiscoverSource>) -> Self {
        Self {
            context: context.clone(),
            source,
            matcher: LibraryMatcher::default(),
            system: Arc::new(SystemLocale::from_env()),
            latest_play: Arc::default(),
            send_order: SendOrder::default(),
        }
    }

    pub fn settings(&self) -> YouTubeSettings {
        self.context.session.settings().youtube.clone()
    }
}
