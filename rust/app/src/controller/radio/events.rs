use feishin_core::queue::Uid;
use feishin_player::PlayerEvent;

use super::RADIO_STREAM_UID;
use crate::controller::{Controller, UiUpdate};

impl Controller {
    pub(in crate::controller) fn on_radio_player_event(&mut self, event: PlayerEvent) {
        match event {
            PlayerEvent::Started { uid } => self.on_station_started(uid),
            PlayerEvent::Position { position_ms, .. } => self.on_station_position(position_ms),
            PlayerEvent::Paused => self.set_playing(false),
            PlayerEvent::Resumed => self.set_playing(true),
            PlayerEvent::TrackEnded { uid } => self.on_station_ended(uid),
            PlayerEvent::Error { uid, message } => self.on_station_error(uid, &message),
            PlayerEvent::Idle => self.on_station_idle(),
            PlayerEvent::StreamTitle(title) => self.on_stream_title(title),
        }
    }

    fn on_station_started(&mut self, uid: Uid) {
        if uid != RADIO_STREAM_UID || self.loading != Some(uid) {
            return;
        }
        self.loading = None;
        self.is_playing = true;
        self.update_media_status();
        self.publish_playback();
    }

    fn on_station_position(&mut self, position_ms: u64) {
        if self.loading.is_some() || self.loaded != Some(RADIO_STREAM_UID) {
            return;
        }
        if let Some(radio) = self.radio.as_mut() {
            radio.elapsed_ms = position_ms;
        }
        self.publish_playback();
    }

    fn on_station_ended(&mut self, uid: Uid) {
        if uid == RADIO_STREAM_UID && self.loaded == Some(uid) {
            self.lose_station(tr!("The radio station stopped streaming"));
        }
    }

    fn on_station_error(&mut self, uid: Option<Uid>, message: &str) {
        if uid.is_some_and(|uid| uid != RADIO_STREAM_UID) {
            return;
        }
        let station_name = self
            .radio
            .as_ref()
            .map(|radio| radio.station.name.clone())
            .unwrap_or_default();
        self.lose_station(tr!("Playing {} failed: {}", station_name, message));
    }

    fn on_station_idle(&mut self) {
        if self.loading.is_none() && self.loaded.is_some() {
            self.set_playing(false);
        }
    }

    fn lose_station(&mut self, message: String) {
        self.loaded = None;
        self.loading = None;
        self.is_playing = false;
        self.update_media_status();
        self.publish_playback();
        self.deps.ui.send(UiUpdate::Error(message));
    }

    fn on_stream_title(&mut self, title: Option<String>) {
        let Some(radio) = self.radio.as_mut() else {
            return;
        };
        if radio.stream_title == title {
            return;
        }
        radio.stream_title = title;
        self.publish_radio();
        self.update_radio_media();
    }
}
