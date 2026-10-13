use std::sync::OnceLock;

use async_trait::async_trait;
use feishin_api::MusicServer;
use feishin_core::model::Song;
use feishin_storage::settings::YouTubeSettings;
use feishin_youtube::music::{
    CollectionKind, CollectionPage, MoodCategory, MusicCollection, MusicError, MusicShelf,
    MusicSong, ShelfItem, chart_country,
};

use super::feed::{DiscoverFeed, FeedNotice, FeedRequest};
use super::source::DiscoverSource;
use crate::demo::{DemoLibrary, FakeServer};

const SHELF_SONGS: usize = 10;
const COLLECTION_SONGS: usize = 15;
const ALBUM_SONGS: usize = 12;
const ALBUM_PREFIX: &str = "MPREdemo";
const PLAYLIST_PREFIX: &str = "VLdemo-";
const MOOD_PREFIX: &str = "demo-mood-";
const ONLY_ON_YOUTUBE_ID: &str = "demoOnlyOnYouTube";
const MOODS: [&str; 8] = [
    "Chill",
    "Focus",
    "Workout",
    "Party",
    "Sleep",
    "Romance",
    "Commute",
    "Feel good",
];
const PLAYLISTS: [&str; 6] = [
    "Morning Mix",
    "Focus Flow",
    "Late Night Drive",
    "Weekend Energy",
    "Rainy Day",
    "Throwback Hits",
];

pub struct DemoDiscover {
    library: DemoLibrary,
    catalog: OnceLock<FakeServer>,
    song_count: usize,
}

impl DemoDiscover {
    pub fn new(library: DemoLibrary) -> Self {
        Self {
            library,
            catalog: OnceLock::new(),
            song_count: library.song_count().max(1),
        }
    }

    fn catalog(&self) -> &FakeServer {
        self.catalog
            .get_or_init(|| FakeServer::with_config(FakeServer::config_for_demo(), self.library))
    }

    fn song(&self, index: usize) -> MusicSong {
        music_song_of(&self.catalog().song_at(index % self.song_count))
    }

    fn picked_songs(&self, seed: u64, count: usize) -> Vec<MusicSong> {
        let start = usize::try_from(seed % 9_973).unwrap_or_default();
        (0..count * 40)
            .map(|step| (start + step * 7_919) % self.song_count)
            .map(|index| self.catalog().song_at(index))
            .filter(|song| song.title.contains(" of the "))
            .take(count)
            .map(|song| music_song_of(&song))
            .collect()
    }

    fn album_songs(&self, first_index: usize) -> Vec<MusicSong> {
        (first_index..first_index + ALBUM_SONGS)
            .filter(|index| *index < self.song_count)
            .map(|index| self.song(index))
            .collect()
    }

    fn album_card(&self, number: usize) -> ShelfItem {
        let first_index = (number * 97 % (self.song_count / ALBUM_SONGS).max(1)) * ALBUM_SONGS;
        let song = self.catalog().song_at(first_index % self.song_count);
        ShelfItem::Collection(MusicCollection {
            browse_id: format!("{ALBUM_PREFIX}{first_index}"),
            kind: CollectionKind::Album,
            title: song.album.clone().unwrap_or_default(),
            subtitle: format!("Album • {}", artist_names(&song)),
            thumbnail_url: Some(demo_thumbnail(&format!("album{first_index}"))),
        })
    }

    fn for_you(&self) -> DiscoverFeed {
        let mut picked: Vec<ShelfItem> = self
            .picked_songs(1, SHELF_SONGS)
            .into_iter()
            .map(ShelfItem::Song)
            .collect();
        picked.push(ShelfItem::Song(only_on_youtube()));
        let playlists = PLAYLISTS
            .iter()
            .map(|name| playlist_card(name, "Playlist • Demo Radio"))
            .collect();
        DiscoverFeed::of_shelves(
            vec![
                shelf("Picked for you", picked),
                shelf("Playlists to try", playlists),
            ],
            FeedNotice::Demo,
        )
    }

    fn new_releases(&self) -> DiscoverFeed {
        let albums = (0..SHELF_SONGS)
            .map(|number| self.album_card(number))
            .collect();
        DiscoverFeed::of_shelves(
            vec![shelf("New albums and singles", albums)],
            FeedNotice::Demo,
        )
    }

    fn charts(&self, country: &str) -> DiscoverFeed {
        let name = chart_country(country).map_or("Global", |found| found.name);
        let charts = ["Top 20 songs", "Trending today", "Top 100 music videos"]
            .iter()
            .map(|chart| playlist_card(&format!("{name} {chart}"), "Chart • Demo Charts"))
            .collect();
        let top_songs = self
            .picked_songs(seed_of(country), SHELF_SONGS)
            .into_iter()
            .map(ShelfItem::Song)
            .collect();
        DiscoverFeed::of_shelves(
            vec![
                shelf("Video charts", charts),
                shelf(&format!("Top songs in {name}"), top_songs),
            ],
            FeedNotice::Demo,
        )
    }

    fn mood(&self, params: &str) -> DiscoverFeed {
        let mood = params
            .strip_prefix(MOOD_PREFIX)
            .and_then(|number| number.parse::<usize>().ok())
            .and_then(|number| MOODS.get(number))
            .copied()
            .unwrap_or("Mixed");
        let playlists = |label: &str| {
            (1..=6)
                .map(|number| {
                    playlist_card(&format!("{mood} {label} {number}"), "Playlist • Demo Radio")
                })
                .collect()
        };
        DiscoverFeed::of_shelves(
            vec![
                shelf(&format!("{mood} picks"), playlists("picks")),
                shelf(&format!("More {mood}"), playlists("mix")),
            ],
            FeedNotice::Demo,
        )
    }

    fn collection_page(&self, browse_id: &str) -> Result<CollectionPage, MusicError> {
        if let Some(first_index) = browse_id
            .strip_prefix(ALBUM_PREFIX)
            .and_then(|number| number.parse::<usize>().ok())
        {
            let songs = self.album_songs(first_index);
            let first = songs.first().cloned().unwrap_or_else(only_on_youtube);
            return Ok(CollectionPage {
                title: first.album.clone().unwrap_or_default(),
                subtitle: format!("{} • Album", first.artist_names()),
                thumbnail_url: Some(demo_thumbnail(&format!("album{first_index}"))),
                songs,
            });
        }
        let playlist = browse_id
            .strip_prefix(PLAYLIST_PREFIX)
            .ok_or_else(|| MusicError::InvalidId(browse_id.to_owned()))?;
        Ok(CollectionPage {
            title: playlist.replace('_', " "),
            subtitle: "Playlist • Demo Radio".to_owned(),
            thumbnail_url: Some(demo_thumbnail(browse_id)),
            songs: self.picked_songs(seed_of(browse_id), COLLECTION_SONGS),
        })
    }
}

fn music_song_of(song: &Song) -> MusicSong {
    let video_id = format!(
        "demo{}",
        song.id
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect::<String>()
    );
    MusicSong {
        thumbnail_url: Some(demo_thumbnail(&video_id)),
        video_id,
        title: song.title.clone(),
        artists: song
            .artists
            .iter()
            .map(|artist| artist.name.clone())
            .collect(),
        album: song.album.clone(),
        duration_ms: Some(song.duration_ms),
    }
}

fn artist_names(song: &Song) -> String {
    song.artists
        .iter()
        .map(|artist| artist.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn only_on_youtube() -> MusicSong {
    MusicSong {
        video_id: ONLY_ON_YOUTUBE_ID.to_owned(),
        title: "Only On YouTube".to_owned(),
        artists: vec!["Demo Busker".to_owned()],
        album: None,
        duration_ms: Some(200_000),
        thumbnail_url: Some(demo_thumbnail(ONLY_ON_YOUTUBE_ID)),
    }
}

fn playlist_card(title: &str, subtitle: &str) -> ShelfItem {
    let id: String = title
        .chars()
        .filter_map(|character| match character {
            ' ' => Some('_'),
            character if character.is_ascii_alphanumeric() => Some(character),
            _ => None,
        })
        .collect();
    ShelfItem::Collection(MusicCollection {
        browse_id: format!("{PLAYLIST_PREFIX}{id}"),
        kind: CollectionKind::Playlist,
        title: title.to_owned(),
        subtitle: subtitle.to_owned(),
        thumbnail_url: Some(demo_thumbnail(&format!("playlist{id}"))),
    })
}

fn shelf(title: &str, items: Vec<ShelfItem>) -> MusicShelf {
    MusicShelf {
        title: title.to_owned(),
        items,
    }
}

fn demo_thumbnail(id: &str) -> String {
    format!("https://demo.invalid/youtube-music/{id}")
}

fn seed_of(text: &str) -> u64 {
    text.bytes().fold(17_u64, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(u64::from(byte))
    })
}

fn mood_categories() -> Vec<MoodCategory> {
    MOODS
        .iter()
        .enumerate()
        .map(|(number, title)| MoodCategory {
            title: (*title).to_owned(),
            params: format!("{MOOD_PREFIX}{number}"),
        })
        .collect()
}

#[async_trait]
impl DiscoverSource for DemoDiscover {
    async fn feed(
        &self,
        request: FeedRequest,
        _settings: YouTubeSettings,
    ) -> Result<DiscoverFeed, MusicError> {
        Ok(match request {
            FeedRequest::ForYou => self.for_you(),
            FeedRequest::NewReleases => self.new_releases(),
            FeedRequest::Charts { country } => self.charts(&country),
            FeedRequest::Moods => DiscoverFeed {
                moods: mood_categories(),
                ..DiscoverFeed::default()
            },
            FeedRequest::Mood { params } => self.mood(&params),
        })
    }

    async fn collection(
        &self,
        browse_id: String,
        _settings: YouTubeSettings,
    ) -> Result<CollectionPage, MusicError> {
        self.collection_page(&browse_id)
    }

    async fn search(
        &self,
        query: String,
        _settings: YouTubeSettings,
    ) -> Result<Vec<MusicSong>, MusicError> {
        let results = self
            .catalog()
            .search(&query, COLLECTION_SONGS)
            .await
            .map_err(|error| MusicError::Network(error.to_string()))?;
        Ok(results.songs.iter().map(music_song_of).collect())
    }
}
