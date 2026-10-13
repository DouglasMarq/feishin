use std::sync::Arc;
use std::time::Duration;

use feishin_api::{ApiError, MusicServer};
use feishin_core::model::LibraryScan;
use slint::{ComponentHandle, Weak};

use super::{AppState, AppWindow, Context, on_ui, show_status, tracks};

const SCAN_POLL_INTERVAL: Duration = Duration::from_secs(2);
const MAX_SCAN_POLLS: usize = 1800;
const NOT_AUTHORIZED_CODE: i32 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanOutcome {
    Finished(LibraryScan),
    StillRunning,
}

pub fn install(window: &AppWindow, ctx: &Context) {
    let weak = window.as_weak();
    let ctx = ctx.clone();
    window.global::<AppState>().on_rescan_library(move || {
        if let Some(window) = weak.upgrade() {
            start_scan(&window, &ctx);
        }
    });
}

fn start_scan(window: &AppWindow, ctx: &Context) {
    let state = window.global::<AppState>();
    if state.get_library_scanning() {
        return;
    }
    let Some(server) = ctx.session.get() else {
        show_status(window, "Not connected to a server");
        return;
    };
    show_progress(window, None);
    let weak = window.as_weak();
    ctx.rt.spawn(async move {
        let outcome = follow_scan(&server, &weak).await;
        on_ui(&weak, move |window| finish_scan(window, outcome));
    });
}

async fn follow_scan(
    server: &Arc<dyn MusicServer>,
    weak: &Weak<AppWindow>,
) -> Result<ScanOutcome, ApiError> {
    server.start_library_scan().await?;
    for _ in 0..MAX_SCAN_POLLS {
        tokio::time::sleep(SCAN_POLL_INTERVAL).await;
        let scan = server.library_scan().await?;
        if !scan.scanning {
            return Ok(ScanOutcome::Finished(scan));
        }
        on_ui(weak, move |window| {
            show_progress(window, scan.scanned_items)
        });
    }
    Ok(ScanOutcome::StillRunning)
}

fn show_progress(window: &AppWindow, scanned_items: Option<u64>) {
    let state = window.global::<AppState>();
    state.set_library_scanning(true);
    let progress = scanned_items.map_or_else(String::new, |count| format!("{count} items so far"));
    state.set_library_scan_progress(progress.into());
}

fn finish_scan(window: &AppWindow, outcome: Result<ScanOutcome, ApiError>) {
    let state = window.global::<AppState>();
    state.set_library_scanning(false);
    state.set_library_scan_progress("".into());
    if matches!(outcome, Ok(ScanOutcome::Finished(_))) {
        tracks::refresh(window);
    }
    show_status(window, &outcome_message(&outcome));
}

fn outcome_message(outcome: &Result<ScanOutcome, ApiError>) -> String {
    match outcome {
        Ok(ScanOutcome::Finished(LibraryScan {
            scanned_items: Some(count),
            ..
        })) => format!("Library scan finished: {count} items"),
        Ok(ScanOutcome::Finished(_)) => "Library scan finished".into(),
        Ok(ScanOutcome::StillRunning) => "The library scan is still running on the server".into(),
        Err(ApiError::Server {
            code: NOT_AUTHORIZED_CODE,
            ..
        }) => "Only a server admin can rescan the library".into(),
        Err(error) => format!("Could not rescan the library: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finished(scanned_items: Option<u64>) -> Result<ScanOutcome, ApiError> {
        Ok(ScanOutcome::Finished(LibraryScan {
            scanning: false,
            scanned_items,
        }))
    }

    #[test]
    fn finished_scans_report_the_item_count_when_known() {
        assert_eq!(
            outcome_message(&finished(Some(42))),
            "Library scan finished: 42 items"
        );
        assert_eq!(outcome_message(&finished(None)), "Library scan finished");
    }

    #[test]
    fn users_without_admin_rights_are_told_why() {
        let refused = Err(ApiError::Server {
            code: NOT_AUTHORIZED_CODE,
            message: "User not authorized".into(),
        });
        assert_eq!(
            outcome_message(&refused),
            "Only a server admin can rescan the library"
        );
    }

    #[test]
    fn other_failures_show_the_error() {
        let failed = Err(ApiError::Network("timed out".into()));
        assert_eq!(
            outcome_message(&failed),
            "Could not rescan the library: network error: timed out"
        );
    }
}
