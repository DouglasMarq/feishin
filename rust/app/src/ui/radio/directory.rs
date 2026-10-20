use anyhow::Result;
use feishin_radio_browser::{StationPage, StationQuery};
use slint::ComponentHandle;

use super::view::RadioView;
use crate::ui::{AppWindow, RadioState, show_status};

enum SearchOutcome {
    Stale,
    Shown,
    Explained(String),
}

fn query_from_form(state: &RadioState<'_>) -> StationQuery {
    StationQuery {
        name: state.get_search_name().to_string(),
        country: state.get_search_country().to_string(),
        tag: state.get_search_tag().to_string(),
        ..StationQuery::default()
    }
}

impl RadioView {
    pub(super) fn search(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let state = window.global::<RadioState>();
        let query = query_from_form(&state);
        if query.is_empty() {
            state.set_results_message(tr!("Enter a name, country or tag to search").into());
            return;
        }
        self.run_search(&window, query);
    }

    pub(super) fn load_more(&self) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let next_page = self.with_model(|model| model.query.next_page());
        self.run_search(&window, next_page);
    }

    fn run_search(&self, window: &AppWindow, query: StationQuery) {
        let generation = self.with_model(|model| model.begin_search(query.clone()));
        window.global::<RadioState>().set_searching(true);
        let view = self.clone();
        self.context.runtime.spawn(async move {
            let found = view.directory.search(&query).await;
            let shown = view.clone();
            view.on_window(move |window| shown.apply_search(window, generation, found));
        });
    }

    pub(super) fn apply_search(
        &self,
        window: &AppWindow,
        generation: u64,
        found: Result<StationPage>,
    ) {
        let outcome = match found {
            Ok(page) => self.page_arrived(generation, page),
            Err(error) => match self.search_failed(window, generation, &error) {
                true => SearchOutcome::Explained(tr!("The radio directory could not be reached")),
                false => SearchOutcome::Stale,
            },
        };
        let state = window.global::<RadioState>();
        match outcome {
            SearchOutcome::Stale => return,
            SearchOutcome::Shown => {}
            SearchOutcome::Explained(message) => state.set_results_message(message.into()),
        }
        state.set_searching(false);
        self.render(window);
    }

    fn page_arrived(&self, generation: u64, page: StationPage) -> SearchOutcome {
        self.with_model(|model| {
            match (
                model.apply_results(generation, page),
                model.results.is_empty(),
            ) {
                (false, _) => SearchOutcome::Stale,
                (true, true) => SearchOutcome::Explained(tr!("No stations found")),
                (true, false) => SearchOutcome::Shown,
            }
        })
    }

    fn search_failed(&self, window: &AppWindow, generation: u64, error: &anyhow::Error) -> bool {
        let current = self.with_model(|model| model.search_failed(generation));
        if current {
            show_status(
                window,
                &tr!(
                    "Searching the radio directory failed: {}",
                    format!("{error:#}")
                ),
            );
        }
        current
    }
}
