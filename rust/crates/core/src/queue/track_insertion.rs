use std::sync::Arc;

use super::{Queue, QueueItem, QueueTrack};

impl Queue {
    pub fn play_tracks_now(
        &mut self,
        tracks: Vec<QueueTrack>,
        start_index: Option<usize>,
    ) -> Option<&QueueItem> {
        self.interner.clear();
        let items = self.items_of_tracks(tracks);
        self.replace_items(items, start_index)
    }

    pub fn add_tracks_next(&mut self, tracks: Vec<QueueTrack>) {
        let items = self.items_of_tracks(tracks);
        self.insert_after_current(items);
    }

    pub fn add_tracks_last(&mut self, tracks: Vec<QueueTrack>) {
        let items = self.items_of_tracks(tracks);
        self.append_items(items);
    }

    fn items_of_tracks(&mut self, tracks: Vec<QueueTrack>) -> Vec<QueueItem> {
        tracks
            .into_iter()
            .map(|track| QueueItem {
                uid: self.allocate_uid(),
                track: Arc::new(self.interner.reintern(track)),
            })
            .collect()
    }
}
