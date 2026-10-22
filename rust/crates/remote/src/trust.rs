use std::net::IpAddr;

use crate::pairing::PairingToken;
use crate::protocol::{DeviceId, DeviceInfo};

pub trait TrustStore: Send + Sync {
    fn trusted_token(&self, device: &DeviceId) -> Option<PairingToken>;
    fn trust(&self, device: &DeviceInfo, token: PairingToken);
    fn forget(&self, device: &DeviceId);
    fn ban(&self, device: &DeviceInfo, address: IpAddr);
    fn is_banned(&self, device: &DeviceId, address: IpAddr) -> bool;
    fn host_token(&self, host: &DeviceId) -> Option<PairingToken>;
    fn remember_host(&self, host: &DeviceId, token: PairingToken);
}
