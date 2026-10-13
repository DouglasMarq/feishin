use std::time::{SystemTime, UNIX_EPOCH};

use feishin_api::MusicServer;
use feishin_core::model::Paging;

use crate::support::{Backend, everything, song_titled, titles};

async fn song_ids(server: &dyn MusicServer, names: &[&str]) -> Vec<String> {
    let mut ids = Vec::new();
    for name in names {
        ids.push(song_titled(server, name).await.id);
    }
    ids
}

async fn playlist_titles(server: &dyn MusicServer, id: &str) -> Vec<String> {
    let songs = server.playlist_songs(id).await.expect("playlist songs");
    titles(&songs).into_iter().map(str::to_owned).collect()
}

async fn listed_playlist_ids(server: &dyn MusicServer) -> Vec<String> {
    server
        .playlist_list(everything())
        .await
        .expect("playlists")
        .items
        .into_iter()
        .map(|playlist| playlist.id)
        .collect()
}

fn unique_name(backend: Backend) -> String {
    let started = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_millis();
    format!("Live {backend:?} Playlist {started}")
}

async fn assert_summary_and_paging(server: &dyn MusicServer, id: &str) {
    let summary = server.playlist(id).await.expect("playlist");
    assert_eq!(summary.song_count, 3);
    let middle = server
        .playlist_song_page(
            id,
            Paging {
                start_index: 1,
                limit: 1,
            },
        )
        .await
        .expect("playlist page");
    assert_eq!(titles(&middle.items), ["Dawn Chorus"]);
    assert_eq!(middle.total_count, Some(3));
}

pub async fn creates_edits_and_deletes_playlists(backend: Backend) {
    let server = backend.connect().await;
    let name = unique_name(backend);
    let initial = song_ids(&*server, &["Blue Note", "Dawn Chorus"]).await;
    let created = server
        .create_playlist(&name, &initial)
        .await
        .expect("create playlist");
    assert_eq!(created.name, name);
    let id = created.id;
    assert!(listed_playlist_ids(&*server).await.contains(&id));
    let appended = song_ids(&*server, &["Late Set"]).await;
    server
        .add_to_playlist(&id, &appended)
        .await
        .expect("add to playlist");
    assert_eq!(
        playlist_titles(&*server, &id).await,
        ["Blue Note", "Dawn Chorus", "Late Set"]
    );
    assert_summary_and_paging(&*server, &id).await;
    server
        .remove_from_playlist(&id, &[0])
        .await
        .expect("remove from playlist");
    assert_eq!(
        playlist_titles(&*server, &id).await,
        ["Dawn Chorus", "Late Set"]
    );
    server.delete_playlist(&id).await.expect("delete playlist");
    assert!(!listed_playlist_ids(&*server).await.contains(&id));
}
