use std::sync::Arc;

use feishin_api::MusicServer;
use feishin_core::model::{RadioStation, ServerType};
use slint::ComponentHandle;
use tokio::sync::oneshot;

use super::freshness::{LoadTicket, LoadedStations};
use super::model::RadioModel;
use super::view::RadioView;
use crate::controller::{Command, SettingsSnapshot};
use crate::ui::{AppWindow, RadioState, show_status};

pub(super) struct ServerStations {
    pub(super) stations: feishin_api::Result<Vec<RadioStation>>,
    pub(super) can_manage: bool,
}

async fn fetch_server_stations(server: Option<Arc<dyn MusicServer>>) -> ServerStations {
    let Some(server) = server else {
        return ServerStations {
            stations: Ok(Vec::new()),
            can_manage: false,
        };
    };
    let (stations, can_manage) =
        tokio::join!(server.radio_stations(), server.can_manage_radio_stations());
    ServerStations {
        stations,
        can_manage: can_manage
            .inspect_err(|error| tracing::debug!("radio permissions unknown: {error}"))
            .unwrap_or(false),
    }
}

async fn saved_favorites(reply: oneshot::Receiver<SettingsSnapshot>) -> Vec<RadioStation> {
    reply
        .await
        .map(|snapshot| snapshot.settings.radio.favorites)
        .inspect_err(|_| tracing::debug!("the controller did not answer with the radio favorites"))
        .unwrap_or_default()
}

fn server_has_radio(server: Option<&Arc<dyn MusicServer>>) -> bool {
    server.is_none_or(|server| server.config().server_type != ServerType::Jellyfin)
}

impl RadioView {
    pub(super) fn load(&self) {
        let server = self.ctx.session.get();
        if let Some(window) = self.window.upgrade() {
            let state = window.global::<RadioState>();
            state.set_stations_loading(true);
            state.set_server_has_radio(server_has_radio(server.as_ref()));
        }
        let ticket = self.with_model(RadioModel::begin_load);
        let (reply, answer) = oneshot::channel();
        self.ctx.send(Command::GetSettings(reply));
        let view = self.clone();
        self.ctx.rt.spawn(async move {
            let favorites = saved_favorites(answer).await;
            let server_stations = fetch_server_stations(server).await;
            let shown = view.clone();
            view.on_window(move |window| {
                shown.apply_loaded(window, ticket, favorites, server_stations);
            });
        });
    }

    pub(super) fn apply_loaded(
        &self,
        window: &AppWindow,
        ticket: LoadTicket,
        favorites: Vec<RadioStation>,
        server_stations: ServerStations,
    ) {
        let (loaded, failure) = match server_stations.stations {
            Ok(stations) => (stations, None),
            Err(error) => (Vec::new(), Some(error)),
        };
        let loaded = LoadedStations {
            favorites,
            server_stations: loaded,
            can_manage: server_stations.can_manage,
        };
        if !self.with_model(|model| model.accept_load(ticket, loaded)) {
            return;
        }
        if let Some(error) = failure {
            show_status(window, &tr!("Could not load the radio stations: {}", error));
        }
        window.global::<RadioState>().set_stations_loading(false);
        self.render(window);
    }
}
