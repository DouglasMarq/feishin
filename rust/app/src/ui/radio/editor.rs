use feishin_core::model::{RadioStation, RadioStationDraft};
use slint::ComponentHandle;

use super::form::draft_from_form;
use super::view::RadioView;
use crate::ui::{AppWindow, RadioSource, RadioState, time_based_id};

fn local_station_id() -> String {
    format!("local-{}", time_based_id())
}

fn saved_station(id: String, draft: RadioStationDraft) -> RadioStation {
    RadioStation {
        id,
        name: draft.name,
        stream_url: draft.stream_url,
        homepage_url: draft.homepage_url,
    }
}

fn fill_editor(state: &RadioState<'_>, source: RadioSource, station: &RadioStation) {
    state.set_edit_id(station.id.as_str().into());
    state.set_edit_source(source);
    state.set_edit_name(station.name.as_str().into());
    state.set_edit_url(station.stream_url.as_str().into());
    state.set_edit_homepage(station.homepage_url.as_deref().unwrap_or_default().into());
    state.set_edit_error("".into());
    state.set_is_editing(true);
}

impl RadioView {
    pub(super) fn edit(&self, source: RadioSource, id: &str) {
        let (Some(window), Some(station)) = (
            self.window.upgrade(),
            self.with_model(|model| model.station(source, id)),
        ) else {
            return;
        };
        fill_editor(&window.global::<RadioState>(), source, &station);
    }

    pub(super) fn save_edit(&self, target: RadioSource) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let state = window.global::<RadioState>();
        let draft = draft_from_form(
            &state.get_edit_name(),
            &state.get_edit_url(),
            &state.get_edit_homepage(),
        );
        let draft = match draft {
            Ok(draft) => draft,
            Err(problem) => return state.set_edit_error(problem.into()),
        };
        let id = state.get_edit_id().to_string();
        match target {
            RadioSource::Server => self.save_on_server(id, draft),
            RadioSource::Favorite | RadioSource::Directory => {
                self.save_on_device(&window, id, draft);
            }
        }
    }

    fn save_on_device(&self, window: &AppWindow, id: String, draft: RadioStationDraft) {
        let id = match id.is_empty() {
            true => local_station_id(),
            false => id,
        };
        self.keep_favorite(saved_station(id, draft));
        window.global::<RadioState>().set_is_editing(false);
    }

    fn save_on_server(&self, id: String, draft: RadioStationDraft) {
        let Some(server) = self.connected_server() else {
            return;
        };
        self.set_busy(true);
        let view = self.clone();
        self.context.runtime.spawn(async move {
            let saved = match id.is_empty() {
                true => server.create_radio_station(&draft).await,
                false => server.update_radio_station(&id, &draft).await,
            };
            let shown = view.clone();
            view.on_window(move |window| shown.finish_server_edit(window, saved));
        });
    }

    fn finish_server_edit(&self, window: &AppWindow, saved: feishin_api::Result<()>) {
        let state = window.global::<RadioState>();
        state.set_is_busy(false);
        match saved {
            Ok(()) => {
                state.set_is_editing(false);
                self.load();
            }
            Err(error) => {
                state.set_edit_error(tr!("The server refused the station: {}", error).into())
            }
        }
    }
}
