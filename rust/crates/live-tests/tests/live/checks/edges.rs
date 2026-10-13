use feishin_api::{ApiError, MusicServer};
use feishin_core::model::{ItemKind, Paging};

use crate::support::{Backend, artist_named, everything, song_titled, titles};

fn assert_not_found<Value: std::fmt::Debug>(lookup: &str, result: feishin_api::Result<Value>) {
    assert!(
        matches!(result, Err(ApiError::NotFound)),
        "{lookup}: expected NotFound, got {result:?}"
    );
}

pub async fn missing_items_are_not_found(backend: Backend) {
    let server = backend.connect().await;
    let missing = |kind| backend.missing_id(kind);
    assert_not_found("song", server.song(missing(ItemKind::Song)).await);
    assert_not_found("album", server.album(missing(ItemKind::Album)).await);
    assert_not_found("artist", server.artist(missing(ItemKind::Artist)).await);
    assert_not_found(
        "playlist",
        server.playlist(missing(ItemKind::Playlist)).await,
    );
}

pub async fn songs_without_lyrics_have_none(backend: Backend) {
    let server = backend.connect().await;
    let song = song_titled(&*server, "Undertow").await;
    assert_eq!(server.lyrics(&song).await.expect("lyrics"), None);
}

pub async fn play_queue_can_be_cleared(backend: Backend) {
    if !backend.clears_play_queue() || !backend.supports_play_queue() {
        return;
    }
    let server = backend.connect().await;
    let song = song_titled(&*server, "Blue Note").await;
    server
        .save_play_queue(std::slice::from_ref(&song.id), Some(&song.id), 0)
        .await
        .expect("save one song");
    server
        .save_play_queue(&[], None, 0)
        .await
        .expect("clear queue");
    let cleared = server.play_queue().await.expect("play queue");
    assert!(
        cleared.is_none_or(|queue| queue.songs.is_empty()),
        "queue was not cleared"
    );
}

async fn created_playlist(server: &dyn MusicServer, name: &str, titles: &[&str]) -> String {
    let mut ids = Vec::new();
    for title in titles {
        ids.push(song_titled(server, title).await.id);
    }
    server
        .create_playlist(name, &ids)
        .await
        .expect("create playlist")
        .id
}

pub async fn empty_playlists_and_multi_removal(backend: Backend) {
    let server = backend.connect().await;
    let empty = created_playlist(&*server, &format!("Empty {backend:?}"), &[]).await;
    assert!(
        server
            .playlist_songs(&empty)
            .await
            .expect("songs")
            .is_empty()
    );
    assert_eq!(
        server.playlist(&empty).await.expect("playlist").song_count,
        0
    );
    server.delete_playlist(&empty).await.expect("delete empty");
    let three = ["Blue Note", "Dawn Chorus", "Late Set"];
    let trimmed = created_playlist(&*server, &format!("Trimmed {backend:?}"), &three).await;
    server
        .remove_from_playlist(&trimmed, &[0, 2])
        .await
        .expect("remove two");
    let remaining = server.playlist_songs(&trimmed).await.expect("songs");
    server
        .delete_playlist(&trimmed)
        .await
        .expect("delete trimmed");
    assert_eq!(titles(&remaining), ["Dawn Chorus"]);
}

pub async fn playlist_list_pages(backend: Backend) {
    let server = backend.connect().await;
    let mut created = Vec::new();
    for index in 0..3 {
        let name = format!("Paged {backend:?} {index}");
        created.push(created_playlist(&*server, &name, &["Undertow"]).await);
    }
    let second = server
        .playlist_list(Paging {
            start_index: 1,
            limit: 1,
        })
        .await
        .expect("playlist page");
    let all = server.playlist_list(everything()).await.expect("playlists");
    for id in &created {
        server.delete_playlist(id).await.expect("delete");
    }
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.start_index, 1);
    assert!(all.items.len() >= 3);
    assert!(
        second
            .total_count
            .is_none_or(|total| total == all.items.len())
    );
}

pub async fn search_respects_the_limit(backend: Backend) {
    let server = backend.connect().await;
    let results = server.search("a", 1).await.expect("search");
    assert!(results.songs.len() <= 1, "{} songs", results.songs.len());
    assert!(results.albums.len() <= 1, "{} albums", results.albums.len());
    assert!(
        results.artists.len() <= 1,
        "{} artists",
        results.artists.len()
    );
}

pub async fn artists_can_be_rated(backend: Backend) {
    if !backend.supports_ratings() {
        return;
    }
    let server = backend.connect().await;
    let artist = artist_named(&*server, "Bravo Sine").await;
    let rated = server.set_rating(ItemKind::Artist, &artist.id, 5).await;
    let detail = server.artist(&artist.id).await.expect("artist");
    server
        .set_rating(ItemKind::Artist, &artist.id, 0)
        .await
        .expect("clear artist rating");
    rated.expect("rate artist");
    assert_eq!(detail.artist.user_rating, Some(5));
}

pub async fn artist_top_songs_belong_to_the_artist(backend: Backend) {
    let server = backend.connect().await;
    let artist = artist_named(&*server, "Charlie Square").await;
    let detail = server.artist(&artist.id).await.expect("artist");
    let top = titles(&detail.top_songs);
    assert!(
        top.iter()
            .all(|title| ["Blue Note", "Late Set"].contains(title)),
        "{top:?}"
    );
}

async fn remove_every_copy_of_the_first_song(server: &dyn MusicServer, playlist: &str) {
    let songs = server.playlist_songs(playlist).await.expect("songs");
    let first = songs.first().expect("a first song").id.clone();
    let copies: Vec<usize> = songs
        .iter()
        .enumerate()
        .filter(|(_, song)| song.id == first)
        .map(|(position, _)| position)
        .collect();
    server
        .remove_from_playlist(playlist, &copies)
        .await
        .expect("remove every copy");
    let remaining = server.playlist_songs(playlist).await.expect("songs");
    assert_eq!(remaining.len(), songs.len() - copies.len());
    assert!(remaining.iter().all(|song| song.id != first));
}

async fn remove_leading_entries(backend: Backend, server: &dyn MusicServer, playlist: &str) {
    let leading: Vec<usize> = (0..42).collect();
    let removed = server.remove_from_playlist(playlist, &leading).await;
    if !backend.has_an_entry_id_per_playlist_position() {
        assert!(matches!(removed, Err(ApiError::Unsupported(_))));
        return;
    }
    removed.expect("remove leading entries");
    let remaining = server.playlist_songs(playlist).await.expect("songs");
    assert_eq!(remaining.len(), 14);
}

pub async fn long_playlists_with_repeats_are_edited(backend: Backend) {
    let server = backend.connect().await;
    let library: Vec<String> = crate::support::all_songs(&*server)
        .await
        .into_iter()
        .map(|song| song.id)
        .collect();
    let repeated: Vec<String> = library.iter().cycle().take(64).cloned().collect();
    let playlist = server
        .create_playlist(&format!("Long {backend:?}"), &repeated)
        .await
        .expect("create long playlist")
        .id;
    assert_eq!(
        server.playlist_songs(&playlist).await.expect("songs").len(),
        64
    );
    remove_every_copy_of_the_first_song(&*server, &playlist).await;
    remove_leading_entries(backend, &*server, &playlist).await;
    server.delete_playlist(&playlist).await.expect("delete");
}

pub async fn streams_transcode_to_opus(backend: Backend) {
    let server = backend.connect().await;
    let song = song_titled(&*server, "Dawn Chorus").await;
    let options = feishin_api::StreamOptions {
        max_bitrate_kbps: Some(96),
        format: Some("opus".into()),
    };
    let stream = crate::support::download(&server.stream_url(&song.id, &options)).await;
    assert!(
        crate::support::is_audio(&stream.bytes),
        "{}",
        stream.content_type
    );
}

pub async fn special_characters_round_trip(backend: Backend) {
    let server = backend.connect().await;
    let spaced = server
        .search("Tidal Pull", 10)
        .await
        .expect("search with a space");
    assert_eq!(titles(&spaced.songs), ["Tidal Pull"]);
    server
        .search("a&b=c+d#e%f", 10)
        .await
        .expect("search with reserved characters");
    let name = format!("Rock & Roll #1 + 50% üñí {backend:?}");
    let song = song_titled(&*server, "Undertow").await;
    let created = server
        .create_playlist(&name, std::slice::from_ref(&song.id))
        .await
        .expect("create playlist");
    let fetched = server.playlist(&created.id).await.expect("playlist");
    server.delete_playlist(&created.id).await.expect("delete");
    assert_eq!(created.name, name);
    assert_eq!(fetched.name, name);
}

pub async fn a_stale_session_token_falls_back(backend: Backend) {
    if !backend.has_native_navidrome_session() {
        return;
    }
    let password = backend.target().password;
    let (_, credential) = backend.login_with(&password).await.expect("login");
    let feishin_api::Credential::Navidrome { subsonic, .. } = credential else {
        panic!("not a Navidrome credential");
    };
    let stale = feishin_api::Credential::Navidrome {
        subsonic,
        nd_token: Some("stale.session.token".into()),
    };
    let server = backend
        .reconnect(backend.login_test_config(), &stale)
        .expect("reconnect");
    let page = server
        .song_list(&Default::default(), everything())
        .await
        .expect("songs after a stale token");
    assert_eq!(page.items.len(), crate::support::SONG_COUNT);
    let count = server.song_count(&Default::default()).await.expect("count");
    assert_eq!(count, crate::support::SONG_COUNT);
}
