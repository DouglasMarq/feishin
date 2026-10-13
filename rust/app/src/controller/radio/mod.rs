mod events;
mod favorites;
mod media;

use feishin_core::model::RadioStation;
use feishin_core::queue::Uid;

use super::{Controller, UiUpdate};
use crate::audio::Track;

pub const RADIO_STREAM_UID: Uid = Uid::MAX;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadioCommand {
    Play(RadioStation),
    Stop,
    AddFavorite(RadioStation),
    RemoveFavorite(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadioNowPlaying {
    pub station_id: String,
    pub station_name: String,
    pub stream_title: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct RadioPlayback {
    station: RadioStation,
    stream_title: Option<String>,
    elapsed_ms: u64,
}

impl RadioPlayback {
    fn new(station: RadioStation) -> Self {
        Self {
            station,
            stream_title: None,
            elapsed_ms: 0,
        }
    }

    fn now_playing(&self) -> RadioNowPlaying {
        RadioNowPlaying {
            station_id: self.station.id.clone(),
            station_name: self.station.name.clone(),
            stream_title: self.stream_title.clone(),
        }
    }

    pub(super) fn elapsed_ms(&self) -> u64 {
        self.elapsed_ms
    }
}

fn stream_track(station: &RadioStation) -> Track {
    Track {
        uid: RADIO_STREAM_UID,
        url: station.stream_url.clone(),
        duration_ms: 0,
    }
}

impl Controller {
    pub(super) fn on_radio_command(&mut self, command: RadioCommand) {
        match command {
            RadioCommand::Play(station) => self.play_station(station),
            RadioCommand::Stop => self.stop_radio(),
            RadioCommand::AddFavorite(station) => self.add_radio_favorite(station),
            RadioCommand::RemoveFavorite(station_id) => self.remove_radio_favorite(&station_id),
        }
    }

    pub(super) fn is_radio_playing(&self) -> bool {
        self.radio.is_some()
    }

    fn play_station(&mut self, station: RadioStation) {
        if !station.has_network_stream_url() {
            self.deps.ui.send(UiUpdate::Error(format!(
                "{} cannot be played: its stream address is not an internet stream",
                station.name
            )));
            return;
        }
        self.discard_preload();
        self.scrobble_tracker.stop();
        let track = stream_track(&station);
        self.radio = Some(RadioPlayback::new(station));
        self.start_audio(&track, 0);
        if self.loaded.is_none() {
            self.leave_radio();
            return;
        }
        self.set_media_live_stream(true);
        self.publish_radio();
        self.update_radio_media();
        self.publish_playback();
    }

    pub(super) fn reload_station(&mut self) {
        let Some(track) = self
            .radio
            .as_ref()
            .map(|radio| stream_track(&radio.station))
        else {
            return;
        };
        self.start_audio(&track, 0);
        self.update_media_status();
        self.publish_playback();
    }

    pub(super) fn pause_station(&mut self) {
        self.close_stream();
        self.update_media_status();
        self.publish_playback();
    }

    fn close_stream(&mut self) {
        if let Err(error) = self.deps.audio.stop() {
            tracing::warn!("stopping the radio stream failed: {error:#}");
        }
        self.loaded = None;
        self.loading = None;
        self.playing = false;
    }

    pub(super) fn stop_radio(&mut self) {
        if self.radio.is_none() {
            return;
        }
        self.close_stream();
        self.leave_radio();
        self.publish_now_playing();
        if let Some(track) = self.current_track() {
            self.update_media_metadata(&track);
        }
        self.update_media_status();
        self.publish_playback();
    }

    pub(super) fn leave_radio(&mut self) {
        if self.radio.take().is_none() {
            return;
        }
        self.set_media_live_stream(false);
        self.deps.ui.send(UiUpdate::Radio(None));
    }

    fn publish_radio(&self) {
        let now_playing = self.radio.as_ref().map(RadioPlayback::now_playing);
        self.deps.ui.send(UiUpdate::Radio(now_playing));
    }
}
