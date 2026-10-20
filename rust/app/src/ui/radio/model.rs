use feishin_core::model::RadioStation;
use feishin_radio_browser::{DirectoryStation, StationPage, StationQuery};

use crate::ui::{RadioRow, RadioSource};

const MAX_DIRECTORY_RESULTS: usize = 500;
const SHOWN_TAGS: usize = 3;

#[derive(Default)]
pub(super) struct RadioModel {
    pub(super) server_stations: Vec<RadioStation>,
    pub(super) favorites: Vec<RadioStation>,
    pub(super) results: Vec<DirectoryStation>,
    pub(super) query: StationQuery,
    pub(super) can_manage: bool,
    pub(super) has_more: bool,
    pub(super) search_generation: u64,
    pub(super) load_generation: u64,
    pub(super) favorites_revision: u64,
}

fn directory_station(station: &DirectoryStation) -> RadioStation {
    RadioStation {
        id: station.uuid.clone(),
        name: station.name.clone(),
        stream_url: station.stream_url.clone(),
        homepage_url: station.homepage_url.clone(),
    }
}

fn joined_details(parts: &[String]) -> String {
    parts
        .iter()
        .filter(|part| !part.is_empty())
        .cloned()
        .reduce(|joined, part| tr!("{} · {}", joined, part))
        .unwrap_or_default()
}

fn audio_details(station: &DirectoryStation) -> String {
    match station.bitrate_kbps {
        Some(bitrate) => tr!("{} {} kbps", station.codec, bitrate).trim().to_owned(),
        None => station.codec.clone(),
    }
}

fn directory_details(station: &DirectoryStation) -> String {
    let tags: Vec<&str> = station
        .tags
        .iter()
        .take(SHOWN_TAGS)
        .map(String::as_str)
        .collect();
    joined_details(&[
        station.country.clone(),
        tags.join(", "),
        audio_details(station),
    ])
}

fn saved_details(place: &str, station: &RadioStation) -> String {
    joined_details(&[place.to_owned(), station.stream_url.clone()])
}

impl RadioModel {
    pub(super) fn is_on_server(&self, stream_url: &str) -> bool {
        self.server_stations
            .iter()
            .any(|station| station.stream_url == stream_url)
    }

    pub(super) fn saved_favorite(&self, station: &RadioStation) -> Option<&RadioStation> {
        self.favorites
            .iter()
            .find(|saved| saved.id == station.id || saved.stream_url == station.stream_url)
    }

    pub(super) fn station(&self, source: RadioSource, id: &str) -> Option<RadioStation> {
        match source {
            RadioSource::Server => find_station(&self.server_stations, id),
            RadioSource::Favorite => find_station(&self.favorites, id),
            RadioSource::Directory => self
                .results
                .iter()
                .find(|station| station.uuid == id)
                .map(directory_station),
        }
    }

    pub(super) fn station_rows(&self) -> Vec<RadioRow> {
        let server_rows = self
            .server_stations
            .iter()
            .map(|station| self.saved_row(station, RadioSource::Server));
        let favorite_rows = self
            .favorites
            .iter()
            .map(|station| self.saved_row(station, RadioSource::Favorite));
        server_rows.chain(favorite_rows).collect()
    }

    pub(super) fn result_rows(&self) -> Vec<RadioRow> {
        self.results
            .iter()
            .map(|station| {
                let as_saved = directory_station(station);
                RadioRow {
                    id: station.uuid.as_str().into(),
                    name: station.name.as_str().into(),
                    details: directory_details(station).into(),
                    source: RadioSource::Directory,
                    is_favorite: self.saved_favorite(&as_saved).is_some(),
                    is_on_server: self.is_on_server(&station.stream_url),
                }
            })
            .collect()
    }

    fn saved_row(&self, station: &RadioStation, source: RadioSource) -> RadioRow {
        let place = match source {
            RadioSource::Server => tr!("On the server"),
            RadioSource::Favorite | RadioSource::Directory => tr!("Saved on this device"),
        };
        RadioRow {
            id: station.id.as_str().into(),
            name: station.name.as_str().into(),
            details: saved_details(&place, station).into(),
            source,
            is_favorite: source == RadioSource::Favorite,
            is_on_server: self.is_on_server(&station.stream_url),
        }
    }

    pub(super) fn begin_search(&mut self, query: StationQuery) -> u64 {
        self.query = query;
        self.search_generation += 1;
        self.search_generation
    }

    pub(super) fn apply_results(&mut self, generation: u64, page: StationPage) -> bool {
        if generation != self.search_generation {
            return false;
        }
        if self.query.offset == 0 {
            self.results.clear();
        }
        let room = MAX_DIRECTORY_RESULTS.saturating_sub(self.results.len());
        self.results.extend(page.stations.into_iter().take(room));
        self.has_more = page.has_more && self.results.len() < MAX_DIRECTORY_RESULTS;
        true
    }

    pub(super) fn search_failed(&mut self, generation: u64) -> bool {
        if generation != self.search_generation {
            return false;
        }
        if self.query.offset == 0 {
            self.results.clear();
            self.has_more = false;
        }
        self.query.offset = self.query.offset.saturating_sub(self.query.limit);
        true
    }

    pub(super) fn remember_favorite(&mut self, station: RadioStation) {
        self.favorites_revision += 1;
        match self
            .favorites
            .iter_mut()
            .find(|saved| saved.id == station.id)
        {
            Some(saved) => *saved = station,
            None => {
                self.favorites
                    .retain(|saved| saved.stream_url != station.stream_url);
                self.favorites.push(station);
            }
        }
    }

    pub(super) fn forget_favorite(&mut self, station_id: &str) {
        self.favorites_revision += 1;
        self.favorites.retain(|saved| saved.id != station_id);
    }
}

fn find_station(stations: &[RadioStation], id: &str) -> Option<RadioStation> {
    stations.iter().find(|station| station.id == id).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn station(id: &str, stream_url: &str) -> RadioStation {
        RadioStation {
            id: id.into(),
            name: format!("Station {id}"),
            stream_url: stream_url.into(),
            homepage_url: None,
        }
    }

    fn found(uuid: &str, stream_url: &str) -> DirectoryStation {
        DirectoryStation {
            uuid: uuid.into(),
            name: format!("Found {uuid}"),
            stream_url: stream_url.into(),
            country: "Germany".into(),
            tags: vec!["jazz".into(), "swing".into(), "bebop".into(), "cool".into()],
            codec: "MP3".into(),
            bitrate_kbps: Some(128),
            ..DirectoryStation::default()
        }
    }

    fn model() -> RadioModel {
        RadioModel {
            server_stations: vec![station("1", "http://a")],
            favorites: vec![station("fav", "http://b")],
            results: vec![
                found("u1", "http://a"),
                found("u2", "http://b"),
                found("u3", "http://c"),
            ],
            ..RadioModel::default()
        }
    }

    #[test]
    fn stations_list_the_server_then_this_device() {
        let rows = model().station_rows();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].source, RadioSource::Server);
        assert_eq!(rows[0].details, "On the server \u{b7} http://a");
        assert!(rows[0].is_on_server && !rows[0].is_favorite);
        assert_eq!(rows[1].source, RadioSource::Favorite);
        assert!(rows[1].is_favorite && !rows[1].is_on_server);
    }

    #[test]
    fn results_know_where_they_are_saved() {
        let rows = model().result_rows();
        let flags: Vec<(bool, bool)> = rows
            .iter()
            .map(|row| (row.is_on_server, row.is_favorite))
            .collect();
        assert_eq!(flags, [(true, false), (false, true), (false, false)]);
        assert_eq!(
            rows[0].details,
            "Germany \u{b7} jazz, swing, bebop \u{b7} MP3 128 kbps"
        );
    }

    #[test]
    fn given_a_station_without_country_tags_or_codec_the_details_skip_the_empty_parts() {
        let bare = DirectoryStation {
            bitrate_kbps: Some(64),
            ..DirectoryStation::default()
        };
        assert_eq!(directory_details(&bare), "64 kbps");
        assert_eq!(directory_details(&DirectoryStation::default()), "");
    }

    #[test]
    fn stations_are_found_by_source_and_id() {
        let model = model();
        assert_eq!(
            model
                .station(RadioSource::Server, "1")
                .map(|found| found.stream_url),
            Some("http://a".into())
        );
        assert_eq!(
            model
                .station(RadioSource::Directory, "u3")
                .map(|found| found.id),
            Some("u3".into())
        );
        assert_eq!(model.station(RadioSource::Favorite, "1"), None);
    }

    fn page(stations: Vec<DirectoryStation>, has_more: bool) -> StationPage {
        StationPage { stations, has_more }
    }

    #[test]
    fn pages_append_until_the_directory_has_no_more_and_stale_pages_are_ignored() {
        let mut model = RadioModel::default();
        let first = model.begin_search(StationQuery {
            limit: 2,
            ..StationQuery::default()
        });
        assert!(model.apply_results(first, page(vec![found("a", "x")], true)));
        assert!(model.has_more);
        let next = model.begin_search(model.query.next_page());
        assert!(!model.apply_results(first, page(vec![found("stale", "z")], true)));
        assert!(model.apply_results(next, page(vec![found("b", "y"), found("c", "z")], false)));
        assert_eq!(model.results.len(), 3);
        assert!(!model.has_more);
    }

    #[test]
    fn results_stop_at_the_cap() {
        let mut model = RadioModel::default();
        let generation = model.begin_search(StationQuery::default());
        let many: Vec<DirectoryStation> = (0..MAX_DIRECTORY_RESULTS + 20)
            .map(|index| found(&index.to_string(), "x"))
            .collect();
        assert!(model.apply_results(generation, page(many, true)));
        assert_eq!(model.results.len(), MAX_DIRECTORY_RESULTS);
        assert!(!model.has_more);
    }

    #[test]
    fn favorites_are_replaced_in_place_and_deduplicated_by_url() {
        let mut model = model();
        model.remember_favorite(RadioStation {
            name: "Renamed".into(),
            ..station("fav", "http://b")
        });
        assert_eq!(model.favorites.len(), 1);
        assert_eq!(model.favorites[0].name, "Renamed");
        model.remember_favorite(station("u2", "http://b"));
        assert_eq!(
            model
                .favorites
                .iter()
                .map(|saved| saved.id.as_str())
                .collect::<Vec<_>>(),
            ["u2"]
        );
        model.forget_favorite("u2");
        assert!(model.favorites.is_empty());
    }
}
