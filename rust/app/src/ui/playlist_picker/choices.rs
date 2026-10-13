use std::collections::HashSet;

use feishin_core::model::{Playlist, Song};

use crate::ui::playlists::song_count_label;
use crate::ui::{PickerRow, format_duration};

pub fn filter_playlists<'playlists>(
    playlists: &'playlists [Playlist],
    filter: &str,
) -> Vec<&'playlists Playlist> {
    let filter = filter.trim().to_lowercase();
    playlists
        .iter()
        .filter(|playlist| filter.is_empty() || playlist.name.to_lowercase().contains(&filter))
        .collect()
}

pub fn new_song_ids(existing: &[Song], song_ids: &[String]) -> Vec<String> {
    let mut seen: HashSet<&str> = existing.iter().map(|song| song.id.as_str()).collect();
    song_ids
        .iter()
        .filter(|song_id| seen.insert(song_id.as_str()))
        .cloned()
        .collect()
}

pub fn picker_row(playlist: &Playlist) -> PickerRow {
    PickerRow {
        id: playlist.id.as_str().into(),
        name: playlist.name.as_str().into(),
        info: format!(
            "{} • {}",
            song_count_label(playlist.song_count as usize),
            format_duration(playlist.duration_ms)
        )
        .into(),
    }
}

#[cfg(test)]
mod tests {
    use feishin_api::MusicServer;

    use super::*;
    use crate::demo::{FakeServer, test_song};
    use crate::ui::playlists::fetch_all;

    #[tokio::test]
    async fn filter_ignores_case_and_whitespace() {
        let server = FakeServer::new(10);
        let playlists = fetch_all(&server).await.unwrap_or_default();
        assert_eq!(filter_playlists(&playlists, "").len(), playlists.len());
        let names: Vec<&str> = filter_playlists(&playlists, "  WORK ")
            .iter()
            .map(|playlist| playlist.name.as_str())
            .collect();
        assert_eq!(names, ["Workout"]);
        assert!(filter_playlists(&playlists, "nothing like this").is_empty());
    }

    #[test]
    fn duplicates_are_skipped_in_order() {
        let existing = [test_song("song-1", 1), test_song("song-2", 1)];
        let song_ids: Vec<String> = ["song-3", "song-1", "song-4", "song-3"]
            .map(String::from)
            .to_vec();
        assert_eq!(new_song_ids(&existing, &song_ids), ["song-3", "song-4"]);
        assert_eq!(new_song_ids(&[], &song_ids), ["song-3", "song-1", "song-4"]);
    }

    #[tokio::test]
    async fn demo_playlists_can_be_edited() {
        let server = FakeServer::new(100);
        let created = server
            .create_playlist("Mine", &["song-1".into(), "song-2".into()])
            .await;
        let Ok(created) = created else {
            panic!("create failed");
        };
        assert_eq!(created.song_count, 2);
        let added = server
            .add_to_playlist(&created.id, &["song-3".into()])
            .await;
        assert!(added.is_ok());
        assert!(server.remove_from_playlist(&created.id, &[0]).await.is_ok());
        let song_ids: Vec<String> = server
            .playlist_songs(&created.id)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|song| song.id)
            .collect();
        assert_eq!(song_ids, ["song-2", "song-3"]);
        assert!(server.delete_playlist(&created.id).await.is_ok());
        assert!(server.playlist_songs(&created.id).await.is_err());
    }
}
