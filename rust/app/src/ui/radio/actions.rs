use std::sync::Arc;

use feishin_api::MusicServer;
use feishin_core::model::{RadioStation, RadioStationDraft};
use slint::ComponentHandle;

use super::view::RadioView;
use crate::controller::{Command, RadioCommand};
use crate::ui::{AppWindow, RadioSource, RadioState, show_status};

impl RadioView {
    pub(super) fn play(&self, source: RadioSource, id: &str) {
        let Some(station) = self.with_model(|model| model.station(source, id)) else {
            return;
        };
        self.context
            .send(Command::Radio(RadioCommand::Play(station)));
        if source == RadioSource::Directory {
            self.count_click(id.to_owned());
        }
    }

    fn count_click(&self, station_uuid: String) {
        let directory = self.directory.clone();
        self.context.runtime.spawn(async move {
            if let Err(error) = directory.count_click(&station_uuid).await {
                tracing::debug!("counting a radio directory click failed: {error:#}");
            }
        });
    }

    pub(super) fn stop(&self) {
        self.context.send(Command::Radio(RadioCommand::Stop));
    }

    pub(super) fn toggle_favorite(&self, source: RadioSource, id: &str) {
        let Some(station) = self.with_model(|model| model.station(source, id)) else {
            return;
        };
        let saved = self.with_model(|model| model.saved_favorite(&station).cloned());
        match saved {
            Some(saved) => self.forget_favorite(&saved.id),
            None => self.keep_favorite(station),
        }
    }

    pub(super) fn keep_favorite(&self, station: RadioStation) {
        self.with_model(|model| model.remember_favorite(station.clone()));
        self.context
            .send(Command::Radio(RadioCommand::AddFavorite(station)));
        self.render_now();
    }

    fn forget_favorite(&self, station_id: &str) {
        self.with_model(|model| model.forget_favorite(station_id));
        self.context
            .send(Command::Radio(RadioCommand::RemoveFavorite(
                station_id.to_owned(),
            )));
        self.render_now();
    }

    pub(super) fn render_now(&self) {
        if let Some(window) = self.window.upgrade() {
            self.render(&window);
        }
    }

    pub(super) fn save_to_server(&self, source: RadioSource, id: &str) {
        let Some(station) = self.with_model(|model| model.station(source, id)) else {
            return;
        };
        let Some(server) = self.connected_server() else {
            return;
        };
        self.set_busy(true);
        let draft = RadioStationDraft::from_station(&station);
        let view = self.clone();
        self.context.runtime.spawn(async move {
            let created = server.create_radio_station(&draft).await;
            let shown = view.clone();
            view.on_window(move |window| {
                shown.finish_change(
                    window,
                    created,
                    &tr!("Could not add {} to the server", draft.name),
                );
            });
        });
    }

    pub(super) fn delete(&self, id: &str) {
        let Some(server) = self.connected_server() else {
            if let Some(window) = self.window.upgrade() {
                window.global::<RadioState>().set_delete_id("".into());
            }
            return;
        };
        self.set_busy(true);
        let id = id.to_owned();
        let view = self.clone();
        self.context.runtime.spawn(async move {
            let deleted = server.delete_radio_station(&id).await;
            let shown = view.clone();
            view.on_window(move |window| {
                window.global::<RadioState>().set_delete_id("".into());
                shown.finish_change(window, deleted, &tr!("Could not delete the station"));
            });
        });
    }

    pub(super) fn connected_server(&self) -> Option<Arc<dyn MusicServer>> {
        let server = self.context.session.get();
        if server.is_none()
            && let Some(window) = self.window.upgrade()
        {
            show_status(&window, &tr!("Not connected to a server"));
        }
        server
    }

    pub(super) fn set_busy(&self, is_busy: bool) {
        if let Some(window) = self.window.upgrade() {
            window.global::<RadioState>().set_is_busy(is_busy);
        }
    }

    pub(super) fn finish_change(
        &self,
        window: &AppWindow,
        outcome: feishin_api::Result<()>,
        failure: &str,
    ) {
        window.global::<RadioState>().set_is_busy(false);
        if let Err(error) = outcome {
            show_status(window, &tr!("{}: {}", failure, error));
        }
        self.load();
    }
}
