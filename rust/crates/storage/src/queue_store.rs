use std::fmt::Display;

use feishin_core::queue::QueueSnapshot;

use crate::{Paths, Result, StorageError, none_when_missing, write_atomic};

pub const QUEUE_FILE: &str = "queue.json";

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct StoredQueue {
    pub snapshot: QueueSnapshot,
    pub position_ms: u64,
}

pub fn save(paths: &Paths, queue: &StoredQueue) -> Result<()> {
    let json = serde_json::to_vec(queue).map_err(invalid_queue_file)?;
    write_atomic(&paths.data_dir.join(QUEUE_FILE), &json)
}

pub fn load(paths: &Paths) -> Result<Option<StoredQueue>> {
    let Some(bytes) = none_when_missing(std::fs::read(paths.data_dir.join(QUEUE_FILE)))? else {
        return Ok(None);
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(invalid_queue_file)
}

pub fn clear(paths: &Paths) -> Result<()> {
    none_when_missing(std::fs::remove_file(paths.data_dir.join(QUEUE_FILE)))?;
    Ok(())
}

fn invalid_queue_file(error: impl Display) -> StorageError {
    StorageError::Invalid(format!("{QUEUE_FILE}: {error}"))
}

#[cfg(test)]
mod tests {
    use feishin_core::model::{ArtistRef, Song};
    use feishin_core::queue::{QueueItem, RepeatMode};

    use super::*;

    fn song(id: &str) -> Song {
        Song {
            id: id.into(),
            server_id: "srv".into(),
            title: format!("Song {id}"),
            artists: vec![ArtistRef {
                id: "ar".into(),
                name: "Artist".into(),
            }],
            album_artists: vec![],
            album: Some("Album".into()),
            album_id: Some("al".into()),
            duration_ms: 180_000,
            track_number: Some(1),
            disc_number: None,
            year: Some(2020),
            genres: vec!["Rock".into()],
            bitrate_kbps: Some(320),
            play_count: 3,
            user_rating: None,
            favorite: true,
            cover_art_id: Some("cov".into()),
            replay_gain: None,
            path: None,
            size_bytes: None,
            container: Some("flac".into()),
            created_at: None,
            last_played_at: None,
        }
    }

    fn sample() -> StoredQueue {
        StoredQueue {
            snapshot: QueueSnapshot {
                items: vec![
                    QueueItem {
                        uid: 1,
                        song: song("a"),
                    },
                    QueueItem {
                        uid: 2,
                        song: song("b"),
                    },
                ],
                shuffled: Some(vec![1, 0]),
                current_uid: Some(2),
                repeat: RepeatMode::All,
                next_uid: 3,
            },
            position_ms: 12_345,
        }
    }

    #[test]
    fn load_without_file_is_none() {
        let directory = tempfile::tempdir().unwrap();
        assert_eq!(load(&Paths::under(directory.path())).unwrap(), None);
    }

    #[test]
    fn round_trip_and_clear() {
        let directory = tempfile::tempdir().unwrap();
        let paths = Paths::under(directory.path());
        let queue = sample();
        save(&paths, &queue).unwrap();
        assert_eq!(load(&paths).unwrap(), Some(queue));
        clear(&paths).unwrap();
        assert_eq!(load(&paths).unwrap(), None);
        clear(&paths).unwrap();
    }

    #[test]
    fn save_replaces_previous_queue() {
        let directory = tempfile::tempdir().unwrap();
        let paths = Paths::under(directory.path());
        save(&paths, &sample()).unwrap();
        let mut other = sample();
        other.snapshot.items.truncate(1);
        other.snapshot.shuffled = None;
        other.position_ms = 0;
        save(&paths, &other).unwrap();
        assert_eq!(load(&paths).unwrap(), Some(other));
        assert_eq!(std::fs::read_dir(&paths.data_dir).unwrap().count(), 1);
    }

    #[test]
    fn corrupt_file_is_invalid() {
        let directory = tempfile::tempdir().unwrap();
        let paths = Paths::under(directory.path());
        std::fs::create_dir_all(&paths.data_dir).unwrap();
        std::fs::write(paths.data_dir.join(QUEUE_FILE), b"{not json").unwrap();
        assert!(matches!(load(&paths), Err(StorageError::Invalid(_))));
    }
}
