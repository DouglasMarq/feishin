use std::collections::HashSet;
use std::hash::Hash;
use std::sync::Arc;

use super::QueueTrack;
use crate::model::{ArtistRef, Song, SongId};

#[derive(Debug, Clone, Default)]
pub(super) struct TrackInterner {
    strings: HashSet<Arc<str>>,
    artist_lists: HashSet<Arc<[ArtistRef]>>,
}

fn intern_borrowed<Value>(pool: &mut HashSet<Arc<Value>>, value: &Value) -> Arc<Value>
where
    Value: Eq + Hash + ?Sized,
    for<'value> Arc<Value>: From<&'value Value>,
{
    if let Some(existing) = pool.get(value) {
        return Arc::clone(existing);
    }
    let interned = Arc::from(value);
    pool.insert(Arc::clone(&interned));
    interned
}

fn intern_shared<Value>(pool: &mut HashSet<Arc<Value>>, value: Arc<Value>) -> Arc<Value>
where
    Value: Eq + Hash + ?Sized,
{
    if let Some(existing) = pool.get(&value) {
        return Arc::clone(existing);
    }
    pool.insert(Arc::clone(&value));
    value
}

impl TrackInterner {
    pub(super) fn track_of(&mut self, song: &Song) -> QueueTrack {
        QueueTrack {
            song_id: SongId::from(song.id.as_str()),
            server_id: intern_borrowed(&mut self.strings, song.server_id.as_str()),
            title: Box::from(song.title.as_str()),
            artists: intern_borrowed(&mut self.artist_lists, song.artists.as_slice()),
            album: self.optional_string(song.album.as_deref()),
            album_id: self.optional_string(song.album_id.as_deref()),
            cover_art_id: self.optional_string(song.cover_art_id.as_deref()),
            duration_ms: song.duration_ms,
        }
    }

    pub(super) fn reintern(&mut self, track: QueueTrack) -> QueueTrack {
        QueueTrack {
            server_id: intern_shared(&mut self.strings, track.server_id),
            artists: intern_shared(&mut self.artist_lists, track.artists),
            album: self.optional_shared(track.album),
            album_id: self.optional_shared(track.album_id),
            cover_art_id: self.optional_shared(track.cover_art_id),
            ..track
        }
    }

    pub(super) fn release_unused(&mut self) {
        self.strings.retain(|value| Arc::strong_count(value) > 1);
        self.artist_lists
            .retain(|value| Arc::strong_count(value) > 1);
    }

    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    fn optional_string(&mut self, value: Option<&str>) -> Option<Arc<str>> {
        value.map(|value| intern_borrowed(&mut self.strings, value))
    }

    fn optional_shared(&mut self, value: Option<Arc<str>>) -> Option<Arc<str>> {
        value.map(|value| intern_shared(&mut self.strings, value))
    }
}
