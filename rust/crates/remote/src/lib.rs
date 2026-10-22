mod client;
mod controllers;
mod discovery;
mod error;
mod frame;
mod host;
mod host_state;
mod lan;
mod pairing;
mod protocol;
mod refusals;
mod trust;

#[cfg(test)]
mod tests;

pub use client::{ClientLink, connect};
pub use controllers::Controller;
pub use discovery::{Discovered, Discovery};
pub use error::RemoteError;
pub use host::{Host, HostDelegate};
pub use host_state::HostState;
pub use pairing::{PairingToken, new_device_id};
pub use protocol::{
    ClientMessage, DeviceId, DeviceInfo, HostMessage, ListPlacement, PREFERRED_PORT, RemoteControl,
    RemoteFailure, RemoteList, RemoteQueue, ServerKey, StateUpdate, TrackPlacement,
    TransferPayload,
};
pub use trust::TrustStore;

pub fn default_device_name() -> String {
    let host_name = gethostname::gethostname().to_string_lossy().into_owned();
    let trimmed = host_name.trim_end_matches(".local").trim();
    match trimmed.is_empty() {
        true => "Cadence".to_owned(),
        false => trimmed.to_owned(),
    }
}
