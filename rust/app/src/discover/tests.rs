use std::sync::Arc;

use feishin_api::MusicServer;
use feishin_core::model::{ArtistRef, Song};
use feishin_storage::settings::YouTubeSettings;
use feishin_youtube::music::{MusicSong, ShelfItem, SystemLocale};
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::library_match::is_confident_match;
use super::sign_in::SignIn;
use super::*;
use crate::demo::{DemoLibrary, FakeServer, test_song};

const HOME: &str = include_str!("../../../crates/youtube/src/music/fixtures/home.json");

fn youtube_song(title: &str, artist: &str, duration_ms: Option<u64>) -> MusicSong {
    MusicSong {
        video_id: "abc123".into(),
        title: title.into(),
        artists: vec![artist.into()],
        album: None,
        duration_ms,
        thumbnail_url: None,
    }
}

fn library_song(title: &str, artist: &str, duration_ms: u64) -> Song {
    Song {
        title: title.into(),
        artists: vec![ArtistRef {
            id: "ar".into(),
            name: artist.into(),
        }],
        ..test_song("s1", duration_ms)
    }
}

#[test]
fn only_the_same_song_by_the_same_artist_is_a_confident_match() {
    let song = youtube_song(
        "Get Lucky (feat. Pharrell Williams)",
        "Daft Punk",
        Some(248_000),
    );
    assert!(is_confident_match(
        &song,
        &library_song("Get Lucky", "daft punk", 250_000)
    ));
    assert!(!is_confident_match(
        &song,
        &library_song("Get Lucky", "Someone Else", 248_000)
    ));
    assert!(!is_confident_match(
        &song,
        &library_song("Get Luckier", "Daft Punk", 248_000)
    ));
    assert!(!is_confident_match(
        &song,
        &library_song("Get Lucky", "Daft Punk", 260_000)
    ));
}

#[test]
fn unknown_durations_do_not_block_a_match() {
    let without_duration = youtube_song("Hey Ya!", "Outkast", None);
    assert!(is_confident_match(
        &without_duration,
        &library_song("Hey Ya", "OutKast", 235_000)
    ));
    let song = youtube_song("Hey Ya!", "Outkast", Some(235_000));
    assert!(is_confident_match(
        &song,
        &library_song("Hey Ya", "Outkast", 0)
    ));
}

#[test]
fn songs_without_artists_never_match() {
    let song = youtube_song("Hey Ya", "", Some(235_000));
    assert!(!is_confident_match(
        &song,
        &library_song("Hey Ya", "", 235_000)
    ));
}

fn demo_server() -> Arc<dyn MusicServer> {
    Arc::new(FakeServer::new(2_400))
}

async fn demo_songs() -> Vec<MusicSong> {
    let demo = DemoDiscover::new(DemoLibrary::with_songs(2_400));
    let feed = demo
        .feed(FeedRequest::ForYou, YouTubeSettings::default())
        .await
        .unwrap();
    feed.shelves[0]
        .items
        .iter()
        .filter_map(|item| match item {
            ShelfItem::Song(song) => Some(song.clone()),
            ShelfItem::Collection(_) => None,
        })
        .collect()
}

#[tokio::test]
async fn library_songs_play_from_the_server_and_the_rest_from_youtube() {
    let songs = demo_songs().await;
    assert_eq!(songs.len(), 11);
    let matcher = LibraryMatcher::default();
    let tracks = resolve_tracks(&matcher, Some(demo_server()), &songs).await;
    let (from_youtube, from_server): (Vec<_>, Vec<_>) =
        tracks.iter().partition(|track| track.is_external());
    assert_eq!(from_server.len(), 10);
    assert_eq!(from_youtube.len(), 1);
    assert_eq!(from_youtube[0].song_id, "yt:demoOnlyOnYouTube");
    assert_eq!(
        tracks.last().map(|track| track.title.as_ref()),
        Some("Only On YouTube")
    );
    let again = matcher.matches(demo_server(), &songs).await;
    assert_eq!(again.iter().filter(|matched| matched.is_some()).count(), 10);
}

#[tokio::test]
async fn presence_tells_library_songs_from_new_ones_and_is_remembered() {
    let songs = demo_songs().await;
    let matcher = LibraryMatcher::default();
    let presence = matcher
        .presence(demo_server(), &songs, std::time::Duration::from_secs(5))
        .await;
    let in_library = presence
        .iter()
        .filter(|presence| **presence == LibraryPresence::InLibrary)
        .count();
    assert_eq!(in_library, 10);
    assert_eq!(presence.last(), Some(&LibraryPresence::NotInLibrary));
    let remembered = matcher
        .presence(demo_server(), &songs, std::time::Duration::ZERO)
        .await;
    assert_eq!(remembered, presence, "answers are remembered");
}

#[tokio::test]
async fn without_a_server_every_song_plays_from_youtube() {
    let songs = demo_songs().await;
    let tracks = resolve_tracks(&LibraryMatcher::default(), None, &songs).await;
    assert!(tracks.iter().all(|track| track.is_external()));
    assert!(tracks.iter().all(|track| track.thumbnail_url().is_some()));
}

#[tokio::test]
async fn the_demo_source_fills_every_section_without_network() {
    let demo = DemoDiscover::new(DemoLibrary::with_songs(2_400));
    let settings = YouTubeSettings::default;
    let releases = demo
        .feed(FeedRequest::NewReleases, settings())
        .await
        .unwrap();
    assert_eq!(releases.shelves[0].items.len(), 10);
    let charts = demo
        .feed(
            FeedRequest::Charts {
                country: "DE".into(),
            },
            settings(),
        )
        .await
        .unwrap();
    assert_eq!(charts.shelves[1].title, "Top songs in Germany");
    let moods = demo.feed(FeedRequest::Moods, settings()).await.unwrap();
    let mood = demo
        .feed(
            FeedRequest::Mood {
                params: moods.moods[1].params.clone(),
            },
            settings(),
        )
        .await
        .unwrap();
    assert_eq!(mood.shelves[0].title, "Focus picks");
}

#[tokio::test]
async fn demo_albums_playlists_and_search_come_from_the_demo_library() {
    let demo = DemoDiscover::new(DemoLibrary::with_songs(2_400));
    let settings = YouTubeSettings::default;
    let releases = demo
        .feed(FeedRequest::NewReleases, settings())
        .await
        .unwrap();
    let ShelfItem::Collection(album) = &releases.shelves[0].items[0] else {
        panic!("expected an album");
    };
    let page = demo
        .collection(album.browse_id.clone(), settings())
        .await
        .unwrap();
    assert_eq!(page.songs.len(), 12);
    assert_eq!(page.title, album.title);
    assert!(
        demo.collection("nonsense".into(), settings())
            .await
            .is_err()
    );
    let beyond_the_library = demo
        .collection(format!("MPREdemo{}", usize::MAX), settings())
        .await
        .unwrap();
    assert!(beyond_the_library.songs.is_empty());
    let found = demo.search("river".into(), settings()).await.unwrap();
    assert!(!found.is_empty());
}

async fn youtube_music_mock() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/browse"))
        .and(body_partial_json(json!({"browseId": "FEmusic_home"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(HOME.as_bytes().to_vec(), "application/json"),
        )
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn without_a_browser_the_home_is_anonymous_with_a_hint() {
    let server = youtube_music_mock().await;
    let source = YouTubeMusicSource::new(SystemLocale::default(), &server.uri(), SignIn::new(None));
    let feed = source
        .feed(FeedRequest::ForYou, YouTubeSettings::default())
        .await
        .unwrap();
    assert_eq!(feed.shelves.len(), 2);
    assert_eq!(feed.notice, FeedNotice::SignInHint);
    let requests = server.received_requests().await.unwrap();
    assert!(requests[0].headers.get("cookie").is_none());
}

#[tokio::test]
async fn a_failed_sign_in_falls_back_to_the_anonymous_home_with_a_warning() {
    let server = youtube_music_mock().await;
    let source = YouTubeMusicSource::new(SystemLocale::default(), &server.uri(), SignIn::new(None));
    let settings = YouTubeSettings {
        cookies_from_browser: Some(feishin_youtube::CookieBrowser::Firefox),
        ..YouTubeSettings::default()
    };
    let feed = source.feed(FeedRequest::ForYou, settings).await.unwrap();
    assert_eq!(feed.shelves.len(), 2);
    assert!(feed.notice.is_warning());
    assert!(feed.notice.message().contains("XDG_RUNTIME_DIR"));
}
