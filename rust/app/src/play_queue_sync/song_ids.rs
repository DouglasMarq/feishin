use std::sync::Arc;

use feishin_core::queue::Queue;

#[derive(Debug, Default)]
pub(super) struct SongIdCache {
    built: Option<BuiltSongIds>,
}

#[derive(Debug)]
struct BuiltSongIds {
    revision: u64,
    item_count: usize,
    server_id: String,
    song_ids: Option<Arc<[String]>>,
}

impl BuiltSongIds {
    fn matches(&self, queue: &Queue, server_id: &str) -> bool {
        self.revision == queue.revision()
            && self.item_count == queue.len()
            && self.server_id == server_id
    }
}

impl SongIdCache {
    pub(super) fn song_ids(&mut self, queue: &Queue, server_id: &str) -> Option<Arc<[String]>> {
        let cached = self
            .built
            .as_ref()
            .filter(|built| built.matches(queue, server_id));
        if let Some(built) = cached {
            return built.song_ids.clone();
        }
        let song_ids = song_ids_on(queue, server_id);
        self.built = Some(BuiltSongIds {
            revision: queue.revision(),
            item_count: queue.len(),
            server_id: server_id.to_owned(),
            song_ids: song_ids.clone(),
        });
        song_ids
    }
}

fn song_ids_on(queue: &Queue, server_id: &str) -> Option<Arc<[String]>> {
    let items = queue.display_order();
    if items.iter().any(|item| &*item.track.server_id != server_id) {
        return None;
    }
    Some(
        items
            .iter()
            .map(|item| item.track.song_id.to_string())
            .collect(),
    )
}
