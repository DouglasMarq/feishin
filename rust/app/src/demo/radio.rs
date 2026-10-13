use std::sync::{MutexGuard, PoisonError};

use feishin_api::{ApiError, Result};
use feishin_core::model::{RadioStation, RadioStationDraft};

use super::server::FakeServer;

const DEMO_STATIONS: [(&str, &str); 4] = [
    ("Demo Jazz Lounge", "https://jazz.example.com"),
    ("Demo Classic Rock", "https://rock.example.com"),
    ("Demo Lo-Fi Beats", "https://lofi.example.com"),
    ("Demo News Talk", "https://news.example.com"),
];

pub(super) struct DemoRadio {
    stations: Vec<RadioStation>,
    next_id: usize,
}

fn demo_station(index: usize, name: &str, homepage_url: &str) -> RadioStation {
    RadioStation {
        id: format!("radio-{index}"),
        name: name.to_owned(),
        stream_url: format!("demo://radio/{index}"),
        homepage_url: Some(homepage_url.to_owned()),
    }
}

impl Default for DemoRadio {
    fn default() -> Self {
        let stations: Vec<RadioStation> = DEMO_STATIONS
            .iter()
            .enumerate()
            .map(|(index, (name, homepage_url))| demo_station(index, name, homepage_url))
            .collect();
        Self {
            next_id: stations.len(),
            stations,
        }
    }
}

fn station_from(id: String, draft: &RadioStationDraft) -> RadioStation {
    RadioStation {
        id,
        name: draft.name.clone(),
        stream_url: draft.stream_url.clone(),
        homepage_url: draft.homepage_url.clone(),
    }
}

impl FakeServer {
    fn demo_radio(&self) -> MutexGuard<'_, DemoRadio> {
        self.radio.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(super) fn demo_radio_stations(&self) -> Vec<RadioStation> {
        self.demo_radio().stations.clone()
    }

    pub(super) fn add_demo_radio_station(&self, draft: &RadioStationDraft) {
        let mut radio = self.demo_radio();
        let id = format!("radio-{}", radio.next_id);
        radio.next_id += 1;
        radio.stations.push(station_from(id, draft));
    }

    pub(super) fn change_demo_radio_station(
        &self,
        id: &str,
        draft: &RadioStationDraft,
    ) -> Result<()> {
        let mut radio = self.demo_radio();
        let station = radio
            .stations
            .iter_mut()
            .find(|station| station.id == id)
            .ok_or(ApiError::NotFound)?;
        *station = station_from(id.to_owned(), draft);
        Ok(())
    }

    pub(super) fn remove_demo_radio_station(&self, id: &str) -> Result<()> {
        let mut radio = self.demo_radio();
        let count_before = radio.stations.len();
        radio.stations.retain(|station| station.id != id);
        match radio.stations.len() < count_before {
            true => Ok(()),
            false => Err(ApiError::NotFound),
        }
    }
}

#[cfg(test)]
mod tests {
    use feishin_api::MusicServer;

    use super::*;

    fn draft(name: &str) -> RadioStationDraft {
        RadioStationDraft {
            name: name.into(),
            stream_url: format!("demo://radio/{name}"),
            homepage_url: None,
        }
    }

    #[tokio::test]
    async fn the_demo_server_has_stations_and_lets_the_user_manage_them() {
        let server = FakeServer::new(10);
        let stations = server.radio_stations().await.unwrap();
        assert_eq!(stations.len(), DEMO_STATIONS.len());
        assert!(server.can_manage_radio_stations().await.unwrap());

        server.create_radio_station(&draft("New")).await.unwrap();
        let created = server.radio_stations().await.unwrap();
        let added = created.last().unwrap();
        assert_eq!(added.name, "New");

        server
            .update_radio_station(&added.id, &draft("Renamed"))
            .await
            .unwrap();
        server.delete_radio_station("radio-0").await.unwrap();
        let names: Vec<String> = server
            .radio_stations()
            .await
            .unwrap()
            .into_iter()
            .map(|station| station.name)
            .collect();
        assert!(names.contains(&"Renamed".to_owned()));
        assert!(!names.contains(&"Demo Jazz Lounge".to_owned()));
        assert!(matches!(
            server.delete_radio_station("radio-0").await,
            Err(ApiError::NotFound)
        ));
    }
}
