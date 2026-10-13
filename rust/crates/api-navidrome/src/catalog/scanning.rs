use feishin_api::Result;
use feishin_core::model::LibraryScan;

use crate::NavidromeServer;
use crate::subsonic::ScanStatusResponse;

impl NavidromeServer {
    pub(crate) async fn begin_library_scan(&self) -> Result<LibraryScan> {
        self.scan_status("startScan").await
    }

    pub(crate) async fn fetch_library_scan(&self) -> Result<LibraryScan> {
        self.scan_status("getScanStatus").await
    }

    async fn scan_status(&self, endpoint: &str) -> Result<LibraryScan> {
        let response = self.subsonic::<ScanStatusResponse>(endpoint, &[]).await?;
        let scan = LibraryScan::from(response.scan_status.unwrap_or_default());
        if !scan.scanning {
            self.forget_fallback_lists();
        }
        Ok(scan)
    }
}
