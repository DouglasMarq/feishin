use std::collections::VecDeque;

use crate::StreamUrl;

const EXPIRY_MARGIN_SECS: u64 = 10 * 60;
const LIFETIME_WITHOUT_EXPIRY_SECS: u64 = 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
struct CachedStream {
    video_id: Box<str>,
    url: String,
    usable_until: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamCache {
    entries: VecDeque<CachedStream>,
    capacity: usize,
}

impl StreamCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn get(&mut self, video_id: &str, now: u64) -> Option<String> {
        self.entries.retain(|entry| entry.usable_until > now);
        self.entries
            .iter()
            .find(|entry| &*entry.video_id == video_id)
            .map(|entry| entry.url.clone())
    }

    pub fn insert(&mut self, video_id: &str, stream: StreamUrl, now: u64) {
        self.remove(video_id);
        let usable_until = usable_until(&stream, now);
        if usable_until <= now || self.capacity == 0 {
            return;
        }
        while self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(CachedStream {
            video_id: Box::from(video_id),
            url: stream.url,
            usable_until,
        });
    }

    pub fn remove(&mut self, video_id: &str) {
        self.entries.retain(|entry| &*entry.video_id != video_id);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn usable_until(stream: &StreamUrl, now: u64) -> u64 {
    match stream.expires_at {
        Some(expires_at) => expires_at.saturating_sub(EXPIRY_MARGIN_SECS),
        None => now.saturating_add(LIFETIME_WITHOUT_EXPIRY_SECS),
    }
}
