use feishin_remote::{DeviceInfo, Discovered};
use tokio::sync::watch;

use super::RemoteService;
use crate::ui::{devices, on_ui};

impl RemoteService {
    pub(super) fn browse(&self) -> Option<watch::Receiver<Vec<Discovered>>> {
        self.discovery
            .as_ref()?
            .browse(self.deps.me.id.clone())
            .inspect_err(|error| tracing::warn!("browsing for other players failed: {error}"))
            .ok()
    }

    pub(super) fn show_devices(&mut self, found: Vec<Discovered>) {
        tracing::info!("found {} other player(s) on the network", found.len());
        self.found = found;
        let devices: Vec<DeviceInfo> = self
            .found
            .iter()
            .map(|found| found.device.clone())
            .collect();
        on_ui(&self.deps.window, move |window| {
            devices::show_devices(window, &devices)
        });
    }
}

pub(super) async fn next_devices(
    devices: &mut Option<watch::Receiver<Vec<Discovered>>>,
) -> Vec<Discovered> {
    loop {
        let Some(receiver) = devices.as_mut() else {
            return std::future::pending().await;
        };
        match receiver.changed().await {
            Ok(()) => return receiver.borrow_and_update().clone(),
            Err(_) => *devices = None,
        }
    }
}
