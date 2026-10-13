use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DirectoryStation {
    pub uuid: String,
    pub name: String,
    pub stream_url: String,
    pub homepage_url: Option<String>,
    pub favicon_url: Option<String>,
    pub country: String,
    pub country_code: String,
    pub tags: Vec<String>,
    pub codec: String,
    pub bitrate_kbps: Option<u32>,
    pub votes: u64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub(crate) struct RawStation {
    stationuuid: String,
    name: String,
    url: String,
    url_resolved: String,
    homepage: String,
    favicon: String,
    tags: String,
    country: String,
    countrycode: String,
    codec: String,
    bitrate: Option<i64>,
    votes: Option<i64>,
}

fn non_empty(text: String) -> Option<String> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn split_tags(tags: &str) -> Vec<String> {
    tags.split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_owned)
        .collect()
}

impl From<RawStation> for DirectoryStation {
    fn from(raw: RawStation) -> Self {
        let stream_url = non_empty(raw.url_resolved)
            .or_else(|| non_empty(raw.url))
            .unwrap_or_default();
        Self {
            uuid: raw.stationuuid,
            name: raw.name.trim().to_owned(),
            stream_url,
            homepage_url: non_empty(raw.homepage),
            favicon_url: non_empty(raw.favicon),
            country: raw.country,
            country_code: raw.countrycode,
            tags: split_tags(&raw.tags),
            codec: raw.codec,
            bitrate_kbps: raw
                .bitrate
                .and_then(|bitrate| u32::try_from(bitrate).ok())
                .filter(|bitrate| *bitrate > 0),
            votes: raw
                .votes
                .and_then(|votes| u64::try_from(votes).ok())
                .unwrap_or(0),
        }
    }
}

pub(crate) fn playable_stations(raw_stations: Vec<RawStation>) -> Vec<DirectoryStation> {
    raw_stations
        .into_iter()
        .map(DirectoryStation::from)
        .filter(|station| !station.stream_url.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_resolved_url_wins_and_tags_are_split() {
        let station = DirectoryStation::from(RawStation {
            stationuuid: "u1".into(),
            name: " Jazz ".into(),
            url: "http://example.com/playlist.pls".into(),
            url_resolved: "http://example.com/stream".into(),
            tags: "jazz, smooth jazz,,".into(),
            bitrate: Some(0),
            votes: Some(-3),
            ..RawStation::default()
        });
        assert_eq!(station.name, "Jazz");
        assert_eq!(station.stream_url, "http://example.com/stream");
        assert_eq!(station.tags, ["jazz", "smooth jazz"]);
        assert_eq!(station.bitrate_kbps, None);
        assert_eq!(station.votes, 0);
        assert_eq!(station.homepage_url, None);
    }

    #[test]
    fn stations_without_any_url_are_dropped() {
        let stations = playable_stations(vec![
            RawStation {
                url: "http://example.com/a".into(),
                ..RawStation::default()
            },
            RawStation::default(),
        ]);
        assert_eq!(stations.len(), 1);
        assert_eq!(
            stations.first().map(|station| station.stream_url.as_str()),
            Some("http://example.com/a")
        );
    }
}
