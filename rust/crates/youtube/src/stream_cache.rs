use std::collections::VecDeque;

use crate::StreamUrl;

const EXPIRY_MARGIN_SECS: u64 = 10 * 60;
const LIFETIME_WITHOUT_EXPIRY_SECS: u64 = 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq)]
struct CachedStream {
    video_id: Box<str>,
    url: String,
    usable_until_unix_seconds: u64,
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

    pub fn get(&self, video_id: &str, now_unix_seconds: u64) -> Option<String> {
        self.entries
            .iter()
            .find(|entry| {
                &*entry.video_id == video_id && entry.usable_until_unix_seconds > now_unix_seconds
            })
            .map(|entry| entry.url.clone())
    }

    pub fn insert(&mut self, video_id: &str, stream: StreamUrl, now_unix_seconds: u64) {
        self.entries.retain(|entry| {
            &*entry.video_id != video_id && entry.usable_until_unix_seconds > now_unix_seconds
        });
        let usable_until_unix_seconds = usable_until_unix_seconds(&stream, now_unix_seconds);
        if usable_until_unix_seconds <= now_unix_seconds || self.capacity == 0 {
            return;
        }
        while self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(CachedStream {
            video_id: Box::from(video_id),
            url: stream.url,
            usable_until_unix_seconds,
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

fn usable_until_unix_seconds(stream: &StreamUrl, now_unix_seconds: u64) -> u64 {
    match stream.expires_at_unix_seconds {
        Some(expires_at_unix_seconds) => expires_at_unix_seconds.saturating_sub(EXPIRY_MARGIN_SECS),
        None => now_unix_seconds.saturating_add(LIFETIME_WITHOUT_EXPIRY_SECS),
    }
}
