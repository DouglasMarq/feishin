use feishin_core::model::Song;

use super::hashing::scramble;
use super::server::FakeServer;

impl FakeServer {
    pub(super) fn random_selection(&self, count: usize) -> Vec<Song> {
        let seed = scramble(count as u64);
        (0..count.min(self.song_count))
            .map(|draw| self.song_at(self.scattered_index(seed, draw)))
            .collect()
    }

    pub(super) fn songs_similar_to(&self, index: usize, count: usize) -> Vec<Song> {
        let seed = scramble(index as u64 ^ 0x51a);
        (0..count.min(self.song_count.saturating_sub(1)))
            .map(|draw| self.scattered_index(seed, draw))
            .filter(|&similar| similar != index)
            .map(|similar| self.song_at(similar))
            .collect()
    }

    fn scattered_index(&self, seed: u64, draw: usize) -> usize {
        (scramble(seed ^ draw as u64) % self.song_count as u64) as usize
    }
}
