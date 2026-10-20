use std::sync::Arc;

use feishin_api::MusicServer;
use feishin_storage::settings::YouTubeSettings;

use super::*;
use crate::demo::{DemoLibrary, FakeServer};
use crate::discover::{DemoDiscover, DiscoverSource, LibraryMatcher};
use crate::store::JsonStore;

fn demo_server() -> Arc<dyn MusicServer> {
    Arc::new(FakeServer::with_config(
        FakeServer::config_for_demo(),
        DemoLibrary::with_songs(2_400),
    ))
}

fn demo_source() -> Arc<dyn DiscoverSource> {
    Arc::new(DemoDiscover::new(DemoLibrary::with_songs(2_400)))
}

fn inputs(today: MixDay, previous: Option<MixSet>) -> MixInputs {
    MixInputs {
        server: demo_server(),
        source: demo_source(),
        settings: YouTubeSettings::default(),
        matcher: LibraryMatcher::default(),
        today,
        previous,
    }
}

fn video_ids(songs: &[feishin_youtube::music::MusicSong]) -> Vec<String> {
    songs.iter().map(|song| song.video_id.clone()).collect()
}

#[tokio::test]
async fn the_demo_library_gets_daily_mixes_and_a_weekly_of_new_songs() {
    let mixes = build_mixes(inputs(MixDay(20_000), None)).await.unwrap();
    assert!(!mixes.daily.is_empty());
    assert!(mixes.daily.len() <= 4);
    let numbers: Vec<u8> = mixes.daily.iter().map(|mix| mix.number).collect();
    assert_eq!(numbers, (1..=numbers.len() as u8).collect::<Vec<_>>());
    let first = mixes.daily.first().unwrap();
    assert!(first.songs.len() >= 12);
    assert!(
        first
            .songs
            .iter()
            .any(|song| !song.video_id.starts_with("demoNew"))
    );
    let weekly = mixes.weekly.clone().unwrap();
    assert!(
        weekly
            .songs
            .iter()
            .all(|song| song.video_id.starts_with("demoNew"))
    );
    assert_eq!(mixes.server_id, demo_server().config().id);
    assert!(mixes.is_fresh_for(&mixes.server_id, MixDay(20_000)));
}

#[tokio::test]
async fn the_weekly_mix_is_kept_for_the_rest_of_its_week() {
    let monday = MixDay(20_004);
    let first = build_mixes(inputs(monday, None)).await.unwrap();
    let tuesday = build_mixes(inputs(MixDay(monday.0 + 1), Some(first.clone())))
        .await
        .unwrap();
    assert_eq!(tuesday.weekly, first.weekly);
    let next_monday = build_mixes(inputs(MixDay(monday.0 + 7), Some(first.clone())))
        .await
        .unwrap();
    let last_week = video_ids(&first.weekly.unwrap().songs);
    if let Some(weekly) = next_monday.weekly {
        assert!(
            video_ids(&weekly.songs)
                .iter()
                .all(|id| !last_week.contains(id))
        );
    }
}

#[tokio::test]
async fn mixes_are_saved_and_loaded_back() {
    let directory = tempfile::tempdir().unwrap();
    let store = JsonStore::new(directory.path());
    assert_eq!(load_mixes(&store), None);
    let mixes = build_mixes(inputs(MixDay(20_000), None)).await.unwrap();
    save_mixes(&store, &mixes);
    assert_eq!(load_mixes(&store), Some(mixes));
    std::fs::write(directory.path().join("discover-mixes.json"), b"{broken").unwrap();
    assert_eq!(load_mixes(&store), None);
}
