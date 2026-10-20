use serde_json::Value;

use super::*;
use crate::music::model::{CollectionKind, MusicShelf, MusicSong, ShelfItem};

fn fixture(name: &str) -> Value {
    let text = match name {
        "home" => include_str!("../fixtures/home.json"),
        "charts" => include_str!("../fixtures/charts.json"),
        "explore" => include_str!("../fixtures/explore.json"),
        "new_releases" => include_str!("../fixtures/new_releases.json"),
        "album" => include_str!("../fixtures/album.json"),
        "playlist" => include_str!("../fixtures/playlist.json"),
        "search" => include_str!("../fixtures/search.json"),
        "mood" => include_str!("../fixtures/mood.json"),
        "next" => include_str!("../fixtures/next.json"),
        other => panic!("unknown fixture {other}"),
    };
    serde_json::from_str(text).unwrap()
}

fn titles(shelves: &[MusicShelf]) -> Vec<&str> {
    shelves.iter().map(|shelf| shelf.title.as_str()).collect()
}

fn collection(item: &ShelfItem) -> (&str, CollectionKind, &str) {
    match item {
        ShelfItem::Collection(collection) => (
            collection.browse_id.as_str(),
            collection.kind,
            collection.title.as_str(),
        ),
        ShelfItem::Song(song) => panic!("expected a collection, got {song:?}"),
    }
}

#[test]
fn home_shelves_hold_songs_and_playlists_and_skip_the_taste_builder() {
    let shelves = browse_shelves(&fixture("home"));
    assert_eq!(
        titles(&shelves),
        ["Escolha a dedo", "Playlists da comunidade em alta"]
    );
    let songs: Vec<&MusicSong> = shelves[0].songs().collect();
    assert_eq!(songs.len(), 4);
    assert_eq!(songs[0].video_id, "9eWo9z6zUi0");
    assert_eq!(songs[0].title, "Peão Todo Tatuado");
    assert_eq!(songs[0].artists, ["Jeninho", "Mariana Fagundes"]);
    assert_eq!(songs[0].album.as_deref(), Some("Peão Todo Tatuado"));
    assert_eq!(songs[0].duration_ms, None);
    assert!(
        songs[0]
            .thumbnail_url
            .as_deref()
            .is_some_and(|url| url.ends_with("=w300-h300-l90-rj"))
    );
    let playlist = collection(&shelves[1].items[0]);
    assert_eq!(
        playlist,
        (
            "VLPLGRY1G015gD0Wn9cQuJqHwyrikKyPb7dZ",
            CollectionKind::Playlist,
            "2000's"
        )
    );
}

#[test]
fn charts_keep_chart_playlists_and_drop_artists() {
    let shelves = browse_shelves(&fixture("charts"));
    assert_eq!(titles(&shelves), ["Paradas de vídeos"]);
    let ids: Vec<&str> = shelves[0]
        .items
        .iter()
        .map(|item| collection(item).0)
        .collect();
    assert_eq!(
        ids,
        [
            "VLOLAK5uy_nxSJNrV0WQ9XbKv_4R9Y98EYwJnZBgkfg",
            "VLPL4fGSI1pDJn42DKxDs-EK72Yqz8rNEvPP",
            "VLPL4fGSI1pDJn4Gs2meaJRo9O8PNYvhjHIg"
        ]
    );
}

#[test]
fn new_releases_are_albums_with_their_subtitle() {
    let shelves = browse_shelves(&fixture("new_releases"));
    assert_eq!(shelves.len(), 1);
    assert_eq!(shelves[0].items.len(), 4);
    let ShelfItem::Collection(album) = &shelves[0].items[0] else {
        panic!("expected an album");
    };
    assert_eq!(album.browse_id, "MPREb_g3H4u5nW8xD");
    assert_eq!(album.kind, CollectionKind::Album);
    assert_eq!(album.subtitle, "EP • Rey Vaqueiro");
    assert!(album.thumbnail_url.is_some());
}

#[test]
fn explore_lists_mood_categories_and_new_release_shelves() {
    let response = fixture("explore");
    let moods = mood_categories(&response);
    assert_eq!(moods.len(), 4);
    assert_eq!(moods[0].title, "Clima alegre");
    assert_eq!(moods[0].params, "ggMPOg1uXzZQbDB5eThLRTQ3");
    assert_eq!(
        titles(&browse_shelves(&response)),
        ["Novos álbuns e singles"]
    );
}

#[test]
fn mood_pages_are_shelves_of_playlists() {
    let shelves = browse_shelves(&fixture("mood"));
    assert_eq!(titles(&shelves), ["Feliz", "Confiante"]);
    assert!(shelves.iter().all(|shelf| shelf.items.len() == 2));
}

#[test]
fn album_tracks_inherit_the_album_name_and_cover() {
    let page = collection_page(&fixture("album"), CollectionKind::Album).unwrap();
    assert_eq!(page.title, "Rey Vaqueiro - Ao Vivo em Belém do Pará");
    assert_eq!(page.subtitle, "Rey Vaqueiro • EP • 2026");
    assert_eq!(page.songs.len(), 3);
    let first = &page.songs[0];
    assert_eq!(first.video_id, "bHn321r_Hqg");
    assert_eq!(first.title, "Príncipe Encantado");
    assert_eq!(first.artists, ["Rey Vaqueiro", "Joelma"]);
    assert_eq!(first.album.as_deref(), Some(page.title.as_str()));
    assert_eq!(first.duration_ms, Some(205_000));
    assert_eq!(first.thumbnail_url, page.thumbnail_url);
}

#[test]
fn playlist_tracks_keep_their_own_thumbnails() {
    let page = collection_page(&fixture("playlist"), CollectionKind::Playlist).unwrap();
    assert_eq!(page.title, "2000's");
    assert_eq!(page.subtitle, "Playlist • 2020");
    let songs: Vec<(&str, &str, Option<u64>)> = page
        .songs
        .iter()
        .map(|song| {
            (
                song.video_id.as_str(),
                song.title.as_str(),
                song.duration_ms,
            )
        })
        .collect();
    assert_eq!(
        songs,
        [
            ("MrTz5xjmso4", "Beautiful Girls", Some(262_000)),
            ("PWgvGjAhvIw", "Hey Ya! (Official HD Video)", Some(304_000)),
            ("VCLxJd1d84s", "Buttons (part. Snoop Dogg)", Some(236_000)),
        ]
    );
    assert_eq!(page.songs[1].artists, ["Outkast"]);
    assert_eq!(page.songs[1].album, None);
    assert!(
        page.songs[1]
            .thumbnail_url
            .as_deref()
            .is_some_and(|url| url.contains("PWgvGjAhvIw"))
    );
}

#[test]
fn search_results_parse_artists_album_and_duration_from_the_details() {
    let response = fixture("search");
    assert!(has_search_sections(&response));
    let songs = search_songs(&response);
    assert_eq!(songs.len(), 4);
    let first = &songs[0];
    assert_eq!(first.video_id, "invented001");
    assert_eq!(
        first.artists,
        ["Velvet Comets", "Juno Marsh", "Otis Fernwood"]
    );
    assert_eq!(first.album.as_deref(), Some("Borrowed Daylight"));
    assert_eq!(first.duration_ms, Some(370_000));
}

#[test]
fn unexpected_documents_parse_to_nothing() {
    let empty = serde_json::json!({"contents": {}});
    assert!(browse_shelves(&empty).is_empty());
    assert!(!has_sections(&empty));
    assert!(collection_page(&empty, CollectionKind::Album).is_none());
    assert!(search_songs(&empty).is_empty());
}

#[test]
fn radio_songs_come_from_the_watch_queue_including_wrapped_items() {
    let response = fixture("next");
    assert!(has_watch_panel(&response));
    let songs = watch_songs(&response);
    let ids: Vec<&str> = songs.iter().map(|song| song.video_id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "invented001",
            "invented002",
            "invented003",
            "invented004",
            "invented005"
        ]
    );
    let Some(first) = songs.first() else {
        panic!("no radio songs");
    };
    assert_eq!(
        first.title,
        "Lucky Static (feat. Juno Marsh and Otis Fernwood)"
    );
    assert_eq!(
        first.artists,
        ["Velvet Comets", "Juno Marsh", "Otis Fernwood"]
    );
    assert_eq!(first.album.as_deref(), Some("Borrowed Daylight"));
    assert_eq!(first.duration_ms, Some(370_000));
    assert!(first.thumbnail_url.is_some());
}

#[test]
fn a_response_without_a_watch_queue_has_no_radio() {
    let empty = serde_json::json!({"contents": {}});
    assert!(!has_watch_panel(&empty));
    assert!(watch_songs(&empty).is_empty());
}
