use feishin_api::Result;
use feishin_core::model::LibraryScan;
use reqwest::Method;

use super::JellyfinServer;
use crate::dto;
use crate::query_params::QueryParams;

const LIBRARY_SCAN_TASK_KEY: &str = "RefreshLibrary";
const IDLE_TASK_STATE: &str = "Idle";

fn library_scan_of(tasks: &[dto::ScheduledTask]) -> LibraryScan {
    let scanning = tasks
        .iter()
        .find(|task| task.key == LIBRARY_SCAN_TASK_KEY)
        .is_some_and(|task| task.state != IDLE_TASK_STATE);
    LibraryScan {
        scanning,
        scanned_items: None,
    }
}

impl JellyfinServer {
    pub(super) async fn begin_library_scan(&self) -> Result<LibraryScan> {
        self.execute(self.request(Method::POST, "library/refresh"))
            .await?;
        Ok(LibraryScan {
            scanning: true,
            scanned_items: None,
        })
    }

    pub(super) async fn fetch_library_scan(&self) -> Result<LibraryScan> {
        let params = QueryParams::default().set("IsHidden", false);
        let tasks: Vec<dto::ScheduledTask> = self.get("scheduledtasks", &params).await?;
        Ok(library_scan_of(&tasks))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(key: &str, state: &str) -> dto::ScheduledTask {
        dto::ScheduledTask {
            key: key.into(),
            state: state.into(),
        }
    }

    #[test]
    fn a_running_library_task_means_scanning() {
        let tasks = [
            task("CleanCache", "Idle"),
            task("RefreshLibrary", "Running"),
        ];
        assert!(library_scan_of(&tasks).scanning);
    }

    #[test]
    fn an_idle_or_missing_library_task_means_done() {
        assert!(!library_scan_of(&[task("RefreshLibrary", "Idle")]).scanning);
        assert!(!library_scan_of(&[task("CleanCache", "Running")]).scanning);
    }
}
