use std::time::Duration;

use crate::support::{Backend, GENRE_NAMES, album_names, artist_names, titles};

pub async fn lists_genres_and_folders(backend: Backend) {
    let server = backend.connect().await;
    let mut genres: Vec<String> = server
        .genre_list()
        .await
        .expect("genres")
        .into_iter()
        .map(|genre| genre.name)
        .collect();
    genres.sort();
    assert_eq!(genres, GENRE_NAMES);
    let folders = server.music_folders().await.expect("music folders");
    assert!(!folders.is_empty());
}

pub async fn searches_songs_albums_and_artists(backend: Backend) {
    let server = backend.connect().await;
    let songs = server.search("Undertow", 10).await.expect("search song");
    assert_eq!(titles(&songs.songs), ["Undertow"]);
    let albums = server.search("Afterglow", 10).await.expect("search album");
    assert_eq!(album_names(&albums.albums), ["Afterglow"]);
    let artists = server.search("Charlie", 10).await.expect("search artist");
    assert_eq!(artist_names(&artists.artists), ["Charlie Square"]);
}

pub async fn library_scan_starts_and_finishes(backend: Backend) {
    let server = backend.connect().await;
    server.start_library_scan().await.expect("start scan");
    for _ in 0..120 {
        let status = server.library_scan().await.expect("scan status");
        if !status.is_scanning {
            return;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    panic!("library scan did not finish within a minute");
}
