use anyhow::Result;
use async_trait::async_trait;
use feishin_radio_browser::{DirectoryStation, StationQuery};

use super::StationDirectory;

const DEMO_DIRECTORY: [(&str, &str, &str, &str); 8] = [
    ("Demo Smooth Jazz", "Germany", "DE", "jazz,smooth jazz"),
    ("Demo Bebop Nights", "United States", "US", "jazz,bebop"),
    (
        "Demo Rock Classics",
        "United Kingdom",
        "GB",
        "rock,classic rock",
    ),
    ("Demo Indie Wave", "Canada", "CA", "indie,rock"),
    ("Demo Lo-Fi Study", "Japan", "JP", "lofi,chillout"),
    ("Demo Ambient Drift", "Iceland", "IS", "ambient,electronic"),
    ("Demo City News", "France", "FR", "news,talk"),
    ("Demo Samba Sol", "Brazil", "BR", "samba,latin"),
];

#[derive(Debug, Clone, Copy, Default)]
pub struct DemoDirectory;

fn demo_station(index: usize, entry: &(&str, &str, &str, &str)) -> DirectoryStation {
    let (name, country, country_code, tags) = *entry;
    DirectoryStation {
        uuid: format!("demo-directory-{index}"),
        name: name.to_owned(),
        stream_url: format!("https://directory.demo.invalid/{index}"),
        homepage_url: None,
        favicon_url: None,
        country: country.to_owned(),
        country_code: country_code.to_owned(),
        tags: tags.split(',').map(str::to_owned).collect(),
        codec: "MP3".to_owned(),
        bitrate_kbps: Some(128),
        votes: u64::try_from(1000 - index * 100).unwrap_or(0),
    }
}

fn contains_ignoring_case(text: &str, filter: &str) -> bool {
    text.to_lowercase().contains(&filter.trim().to_lowercase())
}

fn matches(station: &DirectoryStation, query: &StationQuery) -> bool {
    contains_ignoring_case(&station.name, &query.name)
        && contains_ignoring_case(&station.country, &query.country)
        && (query.tag.trim().is_empty()
            || station
                .tags
                .iter()
                .any(|tag| contains_ignoring_case(tag, &query.tag)))
}

#[async_trait]
impl StationDirectory for DemoDirectory {
    async fn search(&self, query: &StationQuery) -> Result<Vec<DirectoryStation>> {
        Ok(DEMO_DIRECTORY
            .iter()
            .enumerate()
            .map(|(index, entry)| demo_station(index, entry))
            .filter(|station| matches(station, query))
            .skip(query.offset)
            .take(query.limit)
            .collect())
    }

    async fn count_click(&self, _station_uuid: &str) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_demo_directory_filters_by_name_country_and_tag() {
        let jazz = StationQuery {
            tag: "JAZZ".into(),
            ..StationQuery::default()
        };
        let found = DemoDirectory.search(&jazz).await.unwrap();
        assert_eq!(found.len(), 2);

        let german_jazz = StationQuery {
            country: "germ".into(),
            ..jazz
        };
        let found = DemoDirectory.search(&german_jazz).await.unwrap();
        assert_eq!(
            found
                .iter()
                .map(|station| station.name.as_str())
                .collect::<Vec<_>>(),
            ["Demo Smooth Jazz"]
        );

        let paged = StationQuery {
            limit: 3,
            offset: 6,
            ..StationQuery::default()
        };
        assert_eq!(DemoDirectory.search(&paged).await.unwrap().len(), 2);
    }
}
