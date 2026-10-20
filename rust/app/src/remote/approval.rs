use std::sync::{Arc, Mutex};
use std::time::Duration;

use feishin_remote::DeviceInfo;
use tokio::sync::oneshot;

use crate::ui::{AppWindow, devices, on_ui};

const ANSWER_TIMEOUT: Duration = Duration::from_secs(55);

#[derive(Clone, Default)]
pub struct Approvals {
    pending: Arc<Mutex<Option<oneshot::Sender<bool>>>>,
}

impl Approvals {
    pub async fn ask(&self, window: &slint::Weak<AppWindow>, device: &DeviceInfo) -> bool {
        let (answer, answered) = oneshot::channel();
        if let Ok(mut pending) = self.pending.lock() {
            *pending = Some(answer);
        }
        let name = device.name.clone();
        on_ui(window, move |window| {
            devices::show_approval(window, Some(&name))
        });
        let approved = tokio::time::timeout(ANSWER_TIMEOUT, answered)
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or(false);
        self.answer(false);
        on_ui(window, |window| devices::show_approval(window, None));
        approved
    }

    pub fn answer(&self, approved: bool) {
        let pending = self
            .pending
            .lock()
            .ok()
            .and_then(|mut pending| pending.take());
        if let Some(pending) = pending
            && pending.send(approved).is_err()
        {
            tracing::debug!("nobody waited for the device approval");
        }
    }
}
