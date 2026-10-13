use feishin_core::model::RadioStation;

use super::model::RadioModel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct LoadTicket {
    generation: u64,
    favorites_revision: u64,
}

pub(super) struct LoadedStations {
    pub(super) favorites: Vec<RadioStation>,
    pub(super) server_stations: Vec<RadioStation>,
    pub(super) can_manage: bool,
}

impl RadioModel {
    pub(super) fn begin_load(&mut self) -> LoadTicket {
        self.load_generation += 1;
        LoadTicket {
            generation: self.load_generation,
            favorites_revision: self.favorites_revision,
        }
    }

    pub(super) fn accept_load(&mut self, ticket: LoadTicket, loaded: LoadedStations) -> bool {
        if ticket.generation != self.load_generation {
            return false;
        }
        if ticket.favorites_revision == self.favorites_revision {
            self.favorites = loaded.favorites;
        }
        self.server_stations = loaded.server_stations;
        self.can_manage = loaded.can_manage;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn station(id: &str) -> RadioStation {
        RadioStation {
            id: id.into(),
            stream_url: format!("https://{id}.example.com/stream"),
            ..RadioStation::default()
        }
    }

    fn loaded(favorite_ids: &[&str], server_ids: &[&str]) -> LoadedStations {
        LoadedStations {
            favorites: favorite_ids.iter().map(|id| station(id)).collect(),
            server_stations: server_ids.iter().map(|id| station(id)).collect(),
            can_manage: true,
        }
    }

    fn ids(stations: &[RadioStation]) -> Vec<&str> {
        stations.iter().map(|saved| saved.id.as_str()).collect()
    }

    #[test]
    fn an_older_load_never_overwrites_a_newer_one() {
        let mut model = RadioModel::default();
        let older = model.begin_load();
        let newer = model.begin_load();
        assert!(model.accept_load(newer, loaded(&["new"], &["1"])));
        assert!(!model.accept_load(older, loaded(&["old"], &[])));
        assert_eq!(ids(&model.favorites), ["new"]);
        assert_eq!(ids(&model.server_stations), ["1"]);
    }

    #[test]
    fn favorites_changed_during_a_load_are_kept() {
        let mut model = RadioModel::default();
        let ticket = model.begin_load();
        model.remember_favorite(station("added"));
        assert!(model.accept_load(ticket, loaded(&[], &["1"])));
        assert_eq!(ids(&model.favorites), ["added"]);
        assert_eq!(ids(&model.server_stations), ["1"]);
        assert!(model.can_manage);
    }
}
