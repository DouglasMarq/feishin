use std::sync::atomic::{AtomicU64, Ordering};

use feishin_core::model::LibraryScan;

use super::server::FakeServer;

const DEMO_SCAN_POLLS: u64 = 3;

#[derive(Default)]
pub(super) struct DemoScan {
    polls_left: AtomicU64,
}

impl FakeServer {
    pub(super) fn begin_demo_scan(&self) -> LibraryScan {
        self.scan
            .polls_left
            .store(DEMO_SCAN_POLLS, Ordering::Relaxed);
        self.demo_scan_progress(DEMO_SCAN_POLLS)
    }

    pub(super) fn poll_demo_scan(&self) -> LibraryScan {
        let polls_left = self
            .scan
            .polls_left
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |left| {
                left.checked_sub(1)
            })
            .map_or(0, |left| left.saturating_sub(1));
        self.demo_scan_progress(polls_left)
    }

    fn demo_scan_progress(&self, polls_left: u64) -> LibraryScan {
        let song_count = self.library.song_count() as u64;
        let polls_done = DEMO_SCAN_POLLS.saturating_sub(polls_left);
        LibraryScan {
            scanning: polls_left > 0,
            scanned_items: Some(song_count * polls_done / DEMO_SCAN_POLLS),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_demo_scan_finishes_after_a_few_polls() {
        let server = FakeServer::new(900);
        let started = server.begin_demo_scan();
        let polls: Vec<LibraryScan> = (0..4).map(|_| server.poll_demo_scan()).collect();

        assert!(started.scanning);
        assert_eq!(started.scanned_items, Some(0));
        let scanning: Vec<bool> = polls.iter().map(|scan| scan.scanning).collect();
        assert_eq!(scanning, [true, true, false, false]);
        assert_eq!(
            polls.last().and_then(|scan| scan.scanned_items),
            Some(server.library.song_count() as u64)
        );
    }
}
