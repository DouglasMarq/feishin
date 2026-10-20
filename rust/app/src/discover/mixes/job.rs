use std::sync::Arc;

use feishin_api::MusicServer;

use super::builder::{MixInputs, build_mixes};
use super::calendar::MixDay;
use super::model::MixSet;
use super::persist::{load_mixes, save_mixes};
use crate::discover::{DiscoverSource, LibraryMatcher};
use crate::session::Session;
use crate::store::Store;

#[derive(Debug, Clone, PartialEq)]
pub enum MixDelivery {
    Saved(Option<MixSet>),
    Finished(Option<MixSet>),
}

pub struct MixJob {
    pub server: Arc<dyn MusicServer>,
    pub source: Arc<dyn DiscoverSource>,
    pub store: Arc<dyn Store>,
    pub session: Session,
    pub matcher: LibraryMatcher,
    pub today: MixDay,
}

impl MixJob {
    pub async fn run(self, shown: Option<MixSet>, deliver: impl Fn(MixDelivery)) {
        let server_id = self.server.config().id.clone();
        let saved = match shown {
            Some(shown) => Some(shown),
            None => read_saved_mixes(Arc::clone(&self.store)).await,
        };
        let previous = saved.filter(|set| set.is_for_server(&server_id));
        let fresh = previous
            .clone()
            .filter(|set| set.is_fresh_for(&server_id, self.today));
        if let Some(fresh) = fresh {
            deliver(MixDelivery::Finished(Some(fresh)));
            return;
        }
        deliver(MixDelivery::Saved(previous.clone()));
        let built = self.build(previous.clone()).await;
        deliver(MixDelivery::Finished(built.or(previous)));
    }

    async fn build(&self, previous: Option<MixSet>) -> Option<MixSet> {
        let inputs = MixInputs {
            server: Arc::clone(&self.server),
            source: Arc::clone(&self.source),
            settings: self.session.settings().youtube.clone(),
            matcher: self.matcher.clone(),
            today: self.today,
            previous,
        };
        let work = self.session.background_work();
        match work.run(build_mixes(inputs)).await? {
            Ok(mixes) => {
                write_mixes(Arc::clone(&self.store), mixes.clone()).await;
                Some(mixes)
            }
            Err(error) => {
                tracing::warn!("building the Discover mixes failed: {error}");
                None
            }
        }
    }
}

async fn read_saved_mixes(store: Arc<dyn Store>) -> Option<MixSet> {
    tokio::task::spawn_blocking(move || load_mixes(store.as_ref()))
        .await
        .inspect_err(|error| tracing::warn!("the mix loading task stopped: {error}"))
        .ok()
        .flatten()
}

async fn write_mixes(store: Arc<dyn Store>, mixes: MixSet) {
    let written = tokio::task::spawn_blocking(move || save_mixes(store.as_ref(), &mixes)).await;
    if let Err(error) = written {
        tracing::debug!("the mix saving task stopped: {error}");
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::super::model::MIX_FORMAT_VERSION;
    use super::*;
    use crate::demo::{DemoLibrary, FakeServer};
    use crate::discover::DemoDiscover;
    use crate::store::JsonStore;

    const TODAY: MixDay = MixDay(20_000);
    const LIBRARY_SONGS: usize = 2_400;

    fn job(store: &Arc<JsonStore>) -> MixJob {
        let session = Session::default();
        let server: Arc<dyn MusicServer> = Arc::new(FakeServer::with_config(
            FakeServer::config_for_demo(),
            DemoLibrary::with_songs(LIBRARY_SONGS),
        ));
        session.set(Some(Arc::clone(&server)));
        MixJob {
            server,
            source: Arc::new(DemoDiscover::new(DemoLibrary::with_songs(LIBRARY_SONGS))),
            store: Arc::clone(store) as Arc<dyn Store>,
            session,
            matcher: LibraryMatcher::default(),
            today: TODAY,
        }
    }

    async fn deliveries_of(job: MixJob) -> Vec<MixDelivery> {
        let delivered = Mutex::new(Vec::new());
        job.run(None, |delivery| delivered.lock().unwrap().push(delivery))
            .await;
        delivered.into_inner().unwrap()
    }

    fn fresh_mixes() -> MixSet {
        MixSet {
            version: MIX_FORMAT_VERSION,
            server_id: FakeServer::config_for_demo().id,
            day: TODAY,
            daily: Vec::new(),
            weekly: None,
        }
    }

    #[tokio::test]
    async fn given_fresh_saved_mixes_the_job_delivers_them_without_building() {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(JsonStore::new(directory.path()));
        save_mixes(store.as_ref(), &fresh_mixes());

        let deliveries = deliveries_of(job(&store)).await;

        assert_eq!(deliveries, [MixDelivery::Finished(Some(fresh_mixes()))]);
    }

    #[tokio::test]
    async fn given_no_saved_mixes_the_job_builds_and_saves_them() {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(JsonStore::new(directory.path()));

        let deliveries = deliveries_of(job(&store)).await;

        let [MixDelivery::Saved(None), MixDelivery::Finished(Some(built))] = &deliveries[..] else {
            panic!("unexpected deliveries {deliveries:?}");
        };
        assert!(!built.daily.is_empty());
        assert_eq!(load_mixes(store.as_ref()).as_ref(), Some(built));
    }
}
