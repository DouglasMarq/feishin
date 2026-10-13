use std::sync::atomic::{AtomicU64, Ordering};

use feishin_core::model::LibraryScan;

use super::server::FakeServer;

const DEMO_SCAN_POLLS: u64 = 3;

pub(super) struct DemoScan {
    polls_done: AtomicU64,
}

impl Default for DemoScan {
    fn default() -> Self {
        Self {
            polls_done: AtomicU64::new(DEMO_SCAN_POLLS),
        }
    }
}

impl FakeServer {
    pub(super) fn begin_demo_scan(&self) -> LibraryScan {
        self.scan.polls_done.store(0, Ordering::Relaxed);
        self.demo_scan_progress(0)
    }

    pub(super) fn poll_demo_scan(&self) -> LibraryScan {
        let polls_done = self
            .scan
            .polls_done
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1);
        self.demo_scan_progress(polls_done.min(DEMO_SCAN_POLLS))
    }

    fn demo_scan_progress(&self, polls_done: u64) -> LibraryScan {
        let song_count = self.library.song_count() as u64;
        LibraryScan {
            scanning: polls_done < DEMO_SCAN_POLLS,
            scanned_items: Some(song_count * polls_done / DEMO_SCAN_POLLS),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_demo_server_is_idle_before_any_scan() {
        let server = FakeServer::new(900);

        assert!(!server.poll_demo_scan().scanning);
    }

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
