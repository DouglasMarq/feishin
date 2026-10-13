use feishin_core::model::RadioStation;

use crate::controller::Controller;

fn remember(favorites: &mut Vec<RadioStation>, station: RadioStation) {
    match favorites.iter_mut().find(|saved| saved.id == station.id) {
        Some(saved) => *saved = station,
        None => {
            favorites.retain(|saved| saved.stream_url != station.stream_url);
            favorites.push(station);
        }
    }
}

impl Controller {
    pub(super) fn add_radio_favorite(&mut self, station: RadioStation) {
        remember(&mut self.settings.radio.favorites, station);
        self.save_settings();
    }

    pub(super) fn remove_radio_favorite(&mut self, station_id: &str) {
        self.settings
            .radio
            .favorites
            .retain(|saved| saved.id != station_id);
        self.save_settings();
    }
}
