use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use feishin_remote::DeviceInfo;
use tokio::sync::oneshot;

use crate::ui::{AppWindow, devices, on_ui};

const ANSWER_TIMEOUT: Duration = Duration::from_secs(55);

#[derive(Clone, Default)]
pub struct Approvals {
    pending: Arc<Mutex<Option<oneshot::Sender<bool>>>>,
    is_skipped: Arc<AtomicBool>,
}

impl Approvals {
    pub fn skip_asking(&self, is_skipped: bool) {
        self.is_skipped.store(is_skipped, Ordering::Relaxed);
    }

    pub async fn ask(
        &self,
        window: &slint::Weak<AppWindow>,
        device: &DeviceInfo,
        peer: IpAddr,
    ) -> bool {
        if self.is_skipped.load(Ordering::Relaxed) {
            tracing::info!("let {} in without asking", device.name);
            return true;
        }
        let (answer, answered) = oneshot::channel();
        *self.pending.lock().unwrap_or_else(PoisonError::into_inner) = Some(answer);
        let name = device.name.clone();
        on_ui(window, move |window| {
            devices::show_approval(window, Some((&name, peer)))
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

    pub fn answer(&self, is_approved: bool) {
        let pending = self
            .pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(pending) = pending
            && pending.send(is_approved).is_err()
        {
            tracing::debug!("nobody waited for the device approval");
        }
    }
}

#[cfg(test)]
mod tests {
    use feishin_remote::DeviceId;

    use super::*;

    const KITCHEN: [u8; 4] = [192, 168, 0, 7];

    fn phone() -> DeviceInfo {
        DeviceInfo {
            id: DeviceId("phone".into()),
            name: "Phone".into(),
        }
    }

    #[tokio::test]
    async fn given_allow_without_asking_a_new_device_is_paired_without_a_prompt() {
        let approvals = Approvals::default();
        approvals.skip_asking(true);

        let window = slint::Weak::default();
        let device = phone();

        let asked = approvals.ask(&window, &device, IpAddr::from(KITCHEN));

        let approved = tokio::time::timeout(Duration::from_secs(1), asked).await;
        assert_eq!(approved, Ok(true));
        assert!(approvals.pending.lock().unwrap().is_none());
    }

    #[tokio::test]
    async fn given_allow_without_asking_is_off_the_prompt_still_decides() {
        let approvals = Approvals::default();
        approvals.skip_asking(true);
        approvals.skip_asking(false);
        let answering = approvals.clone();

        let asked = tokio::spawn(async move {
            approvals
                .ask(&slint::Weak::default(), &phone(), IpAddr::from(KITCHEN))
                .await
        });
        while answering.pending.lock().unwrap().is_none() {
            tokio::task::yield_now().await;
        }
        answering.answer(false);

        assert!(!asked.await.unwrap());
    }
}
