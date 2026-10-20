use crate::pairing::PairingToken;
use crate::protocol::{DeviceId, DeviceInfo};

pub trait TrustStore: Send + Sync {
    fn trusted_token(&self, device: &DeviceId) -> Option<PairingToken>;
    fn trust(&self, device: &DeviceInfo, token: PairingToken);
    fn host_token(&self, host: &DeviceId) -> Option<PairingToken>;
    fn remember_host(&self, host: &DeviceId, token: PairingToken);
}
