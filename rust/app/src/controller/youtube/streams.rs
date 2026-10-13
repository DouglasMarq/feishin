use feishin_core::queue::Uid;
use feishin_youtube::{StreamCache, StreamUrl, YouTubeError, unix_now};
use tokio::task::AbortHandle;

const CACHED_STREAMS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvePurpose {
    Load { request: u64, start_ms: u64 },
    Preload,
}

#[derive(Debug)]
pub struct YouTubeResolution {
    pub(super) uid: Uid,
    pub(super) video_id: Box<str>,
    pub(super) purpose: ResolvePurpose,
    pub(super) result: Result<StreamUrl, YouTubeError>,
}

struct PendingTask {
    uid: Uid,
    request: u64,
    task: AbortHandle,
}

pub(in crate::controller) struct YouTubeStreams {
    cache: StreamCache,
    load: Option<PendingTask>,
    preload: Option<PendingTask>,
    failed_preload: Option<Uid>,
    requests: u64,
    consecutive_failures: usize,
}

impl Default for YouTubeStreams {
    fn default() -> Self {
        Self {
            cache: StreamCache::new(CACHED_STREAMS),
            load: None,
            preload: None,
            failed_preload: None,
            requests: 0,
            consecutive_failures: 0,
        }
    }
}

fn abort(pending: Option<PendingTask>) {
    if let Some(pending) = pending {
        pending.task.abort();
    }
}

impl YouTubeStreams {
    pub(in crate::controller) fn cached_url(&self, video_id: &str) -> Option<String> {
        self.cache.get(video_id, unix_now())
    }

    pub(super) fn remember(&mut self, video_id: &str, stream: StreamUrl) {
        self.cache.insert(video_id, stream, unix_now());
    }

    pub(super) fn forget(&mut self, video_id: &str) {
        self.cache.remove(video_id);
    }

    pub(super) fn next_request(&mut self) -> u64 {
        self.requests = self.requests.wrapping_add(1);
        self.requests
    }

    pub(super) fn track_load(&mut self, uid: Uid, request: u64, task: AbortHandle) {
        abort(self.load.replace(PendingTask { uid, request, task }));
    }

    pub(in crate::controller) fn cancel_load(&mut self) {
        abort(self.load.take());
    }

    pub(in crate::controller) fn is_loading(&self) -> bool {
        self.load.is_some()
    }

    pub(super) fn finish_load(&mut self, uid: Uid, request: u64) -> bool {
        let matches = self
            .load
            .as_ref()
            .is_some_and(|pending| pending.uid == uid && pending.request == request);
        if matches {
            self.load = None;
        }
        matches
    }

    pub(super) fn wants_preload(&self, uid: Uid) -> bool {
        let pending = self
            .preload
            .as_ref()
            .is_some_and(|pending| pending.uid == uid);
        !pending && self.failed_preload != Some(uid)
    }

    pub(super) fn track_preload(&mut self, uid: Uid, task: AbortHandle) {
        let request = self.next_request();
        abort(self.preload.replace(PendingTask { uid, request, task }));
    }

    pub(super) fn finish_preload(&mut self, uid: Uid) {
        if self
            .preload
            .as_ref()
            .is_some_and(|pending| pending.uid == uid)
        {
            self.preload = None;
        }
    }

    pub(super) fn preload_failed(&mut self, uid: Uid) {
        self.failed_preload = Some(uid);
    }

    pub(super) fn forget_failed_preload(&mut self) {
        self.failed_preload = None;
    }

    pub(super) fn count_failure(&mut self) -> usize {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        self.consecutive_failures
    }

    pub(in crate::controller) fn reset_failures(&mut self) {
        self.consecutive_failures = 0;
    }
}
