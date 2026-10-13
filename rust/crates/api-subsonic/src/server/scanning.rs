use feishin_api::Result;
use feishin_core::model::LibraryScan;

use super::SubsonicServer;
use crate::client::Params;
use crate::types::{ScanStatus, ScanStatusResponse};

fn library_scan_of(status: ScanStatus) -> LibraryScan {
    LibraryScan {
        scanning: status.scanning,
        scanned_items: status
            .count
            .filter(|count| *count >= 0.0)
            .map(|count| count as u64),
    }
}

impl SubsonicServer {
    pub(super) async fn begin_library_scan(&self) -> Result<LibraryScan> {
        self.scan_status("startScan.view").await
    }

    pub(super) async fn fetch_library_scan(&self) -> Result<LibraryScan> {
        self.scan_status("getScanStatus.view").await
    }

    async fn scan_status(&self, endpoint: &str) -> Result<LibraryScan> {
        let response: ScanStatusResponse = self.fetch(endpoint, Params::new()).await?;
        let scan = library_scan_of(response.scan_status.unwrap_or_default());
        if !scan.scanning {
            self.list_caches.forget_annotations();
            self.list_caches.forget_playlists();
        }
        Ok(scan)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_counts_are_ignored() {
        let status = ScanStatus {
            scanning: true,
            count: Some(-1.0),
        };
        assert_eq!(
            library_scan_of(status),
            LibraryScan {
                scanning: true,
                scanned_items: None
            }
        );
    }

    #[test]
    fn counts_become_scanned_items() {
        let status = ScanStatus {
            scanning: false,
            count: Some(1234.0),
        };
        assert_eq!(library_scan_of(status).scanned_items, Some(1234));
    }
}
