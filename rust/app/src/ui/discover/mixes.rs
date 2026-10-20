use std::sync::Arc;

use feishin_api::MusicServer;
use feishin_youtube::music::{CollectionPage, MusicShelf};
use slint::ComponentHandle;

use super::deps::DiscoverDeps;
use super::model::with_discover;
use super::render::render_sections;
use crate::discover::mixes::{
    MixDay, MixId, MixInputs, MixSet, build_mixes, load_mixes, save_mixes,
};
use crate::store::Store;
use crate::ui::{AppWindow, on_ui};

#[derive(Default)]
pub struct MixState {
    set: Option<MixSet>,
    building: bool,
    revision: u64,
}

impl MixState {
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn shelf(&self) -> Option<MusicShelf> {
        self.set.as_ref()?.shelf()
    }

    pub fn page(&self, id: MixId) -> Option<CollectionPage> {
        self.set.as_ref()?.page(id)
    }

    fn begin_build(&mut self, server_id: &str, today: MixDay) -> Option<Option<MixSet>> {
        let fresh = self
            .set
            .as_ref()
            .is_some_and(|set| set.is_fresh_for(server_id, today));
        if self.building || fresh {
            return None;
        }
        if self
            .set
            .as_ref()
            .is_some_and(|set| !set.is_for_server(server_id))
        {
            self.show(None);
        }
        self.building = true;
        Some(self.set.clone())
    }

    fn show(&mut self, set: Option<MixSet>) {
        if self.set != set {
            self.set = set;
            self.revision += 1;
        }
    }

    fn finish_build(&mut self, set: Option<MixSet>) {
        self.building = false;
        self.show(set);
    }
}

pub fn mix_page(window: &AppWindow, id: MixId) -> Option<CollectionPage> {
    with_discover(window, |model| model.mixes.borrow().page(id)).flatten()
}

pub fn ensure_mixes(window: &AppWindow, deps: &DiscoverDeps) {
    let Some(server) = deps.ctx.session.get() else {
        return;
    };
    let server_id = server.config().id.clone();
    let today = MixDay::today();
    let started = with_discover(window, |model| {
        model.mixes.borrow_mut().begin_build(&server_id, today)
    })
    .flatten();
    let Some(shown) = started else {
        return;
    };
    let job = MixJob {
        weak_window: window.as_weak(),
        deps: deps.clone(),
        server,
        server_id,
        today,
    };
    deps.ctx.rt.spawn(job.run(shown));
}

struct MixJob {
    weak_window: slint::Weak<AppWindow>,
    deps: DiscoverDeps,
    server: Arc<dyn MusicServer>,
    server_id: String,
    today: MixDay,
}

impl MixJob {
    async fn run(self, shown: Option<MixSet>) {
        let saved = match shown {
            Some(shown) => Some(shown),
            None => read_saved_mixes(Arc::clone(&self.deps.ctx.store)).await,
        };
        let previous = saved.filter(|set| set.is_for_server(&self.server_id));
        let fresh = previous
            .clone()
            .filter(|set| set.is_fresh_for(&self.server_id, self.today));
        if let Some(fresh) = fresh {
            deliver(&self.weak_window, &self.deps, Some(fresh), true);
            return;
        }
        deliver(&self.weak_window, &self.deps, previous.clone(), false);
        let built = self.build(previous.clone()).await;
        deliver(&self.weak_window, &self.deps, built.or(previous), true);
    }

    async fn build(&self, previous: Option<MixSet>) -> Option<MixSet> {
        let inputs = MixInputs {
            server: Arc::clone(&self.server),
            source: Arc::clone(&self.deps.source),
            settings: self.deps.settings(),
            matcher: self.deps.matcher.clone(),
            today: self.today,
            previous,
        };
        let work = self.deps.ctx.session.background_work();
        match work.run(build_mixes(inputs)).await? {
            Ok(mixes) => {
                write_mixes(Arc::clone(&self.deps.ctx.store), mixes.clone()).await;
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
        .ok()
        .flatten()
}

async fn write_mixes(store: Arc<dyn Store>, mixes: MixSet) {
    let written = tokio::task::spawn_blocking(move || save_mixes(store.as_ref(), &mixes)).await;
    if let Err(error) = written {
        tracing::debug!("the mix saving task stopped: {error}");
    }
}

fn deliver(
    weak_window: &slint::Weak<AppWindow>,
    deps: &DiscoverDeps,
    mixes: Option<MixSet>,
    finished: bool,
) {
    let session = deps.ctx.session.clone();
    on_ui(weak_window, move |window| {
        let current_server = session.get().map(|server| server.config().id.clone());
        let mixes = mixes.filter(|set| Some(&set.server_id) == current_server.as_ref());
        with_discover(window, |model| {
            let mut state = model.mixes.borrow_mut();
            match finished {
                true => state.finish_build(mixes),
                false => state.show(mixes),
            }
        });
        render_sections(window);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discover::mixes::MixSet;

    fn mixes_for(server_id: &str, day: MixDay) -> MixSet {
        serde_json::from_value(serde_json::json!({
            "version": 1,
            "server_id": server_id,
            "day": day.0,
            "daily": [],
            "weekly": null
        }))
        .unwrap()
    }

    #[test]
    fn a_build_starts_once_and_not_while_the_mixes_are_fresh() {
        let mut state = MixState::default();
        assert_eq!(state.begin_build("server", MixDay(5)), Some(None));
        assert_eq!(
            state.begin_build("server", MixDay(5)),
            None,
            "already building"
        );
        state.finish_build(Some(mixes_for("server", MixDay(5))));
        assert_eq!(state.revision(), 1);
        assert_eq!(state.begin_build("server", MixDay(5)), None, "fresh");
        assert!(
            state.begin_build("server", MixDay(6)).is_some(),
            "a new day"
        );
    }

    #[test]
    fn mixes_of_another_server_are_hidden_before_rebuilding() {
        let mut state = MixState::default();
        state.finish_build(Some(mixes_for("old", MixDay(5))));
        assert_eq!(state.begin_build("new", MixDay(5)), Some(None));
        assert!(state.set.is_none());
        assert_eq!(state.revision(), 2);
    }
}
