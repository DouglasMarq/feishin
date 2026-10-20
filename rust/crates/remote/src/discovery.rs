use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Mutex;

use mdns_sd::{IfKind, ResolvedService, ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::sync::watch;

use crate::error::RemoteError;
use crate::lan::is_lan_peer;
use crate::protocol::{DeviceId, DeviceInfo, PROTOCOL_VERSION, SERVICE_TYPE};

const ID_KEY: &str = "id";
const NAME_KEY: &str = "name";
const VERSION_KEY: &str = "v";
const MAX_DISCOVERED_DEVICES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovered {
    pub device: DeviceInfo,
    pub address: SocketAddr,
}

pub struct Discovery {
    daemon: ServiceDaemon,
    advertised: Mutex<Option<String>>,
}

impl Discovery {
    pub fn start() -> Result<Self, RemoteError> {
        let daemon = ServiceDaemon::new().map_err(discovery_error)?;
        daemon
            .disable_interface(IfKind::IPv6)
            .map_err(discovery_error)?;
        Ok(Self {
            daemon,
            advertised: Mutex::new(None),
        })
    }

    pub fn advertise(&self, device: &DeviceInfo, port: u16) -> Result<(), RemoteError> {
        self.withdraw();
        let properties = [
            (ID_KEY, device.id.0.as_str()),
            (NAME_KEY, device.name.as_str()),
            (VERSION_KEY, &PROTOCOL_VERSION.to_string()),
        ];
        let instance = format!("{} ({})", device.name, device.id.0);
        let host_name = format!("feishin-{}.local.", device.id.0);
        let service = ServiceInfo::new(
            SERVICE_TYPE,
            &instance,
            &host_name,
            "",
            port,
            &properties[..],
        )
        .map_err(discovery_error)?
        .enable_addr_auto();
        let fullname = service.get_fullname().to_owned();
        self.daemon.register(service).map_err(discovery_error)?;
        if let Ok(mut advertised) = self.advertised.lock() {
            *advertised = Some(fullname);
        }
        Ok(())
    }

    pub fn withdraw(&self) {
        let Some(fullname) = self
            .advertised
            .lock()
            .ok()
            .and_then(|mut advertised| advertised.take())
        else {
            return;
        };
        if let Err(error) = self.daemon.unregister(&fullname) {
            tracing::debug!("withdrawing the remote control announcement failed: {error}");
        }
    }

    pub fn browse(&self, me: DeviceId) -> Result<watch::Receiver<Vec<Discovered>>, RemoteError> {
        let events = self.daemon.browse(SERVICE_TYPE).map_err(discovery_error)?;
        let (sender, receiver) = watch::channel(Vec::new());
        tokio::spawn(async move {
            let mut found: HashMap<String, Discovered> = HashMap::new();
            while let Ok(event) = events.recv_async().await {
                let changed = match event {
                    ServiceEvent::ServiceResolved(service) => discovered_of(&service, &me)
                        .is_some_and(|device| {
                            remember(&mut found, service.get_fullname().to_owned(), device)
                        }),
                    ServiceEvent::ServiceRemoved(_service_type, fullname) => {
                        found.remove(&fullname).is_some()
                    }
                    _ => false,
                };
                if changed && sender.send(sorted(&found)).is_err() {
                    return;
                }
            }
        });
        Ok(receiver)
    }
}

impl Drop for Discovery {
    fn drop(&mut self) {
        self.withdraw();
        if let Err(error) = self.daemon.shutdown() {
            tracing::debug!("stopping device discovery failed: {error}");
        }
    }
}

fn remember(found: &mut HashMap<String, Discovered>, fullname: String, device: Discovered) -> bool {
    if found.len() >= MAX_DISCOVERED_DEVICES && !found.contains_key(&fullname) {
        tracing::debug!("ignoring a player announcement past {MAX_DISCOVERED_DEVICES} players");
        return false;
    }
    found.insert(fullname, device);
    true
}

fn sorted(found: &HashMap<String, Discovered>) -> Vec<Discovered> {
    let mut devices: Vec<Discovered> = found.values().cloned().collect();
    devices.sort_by(|left, right| {
        left.device
            .name
            .cmp(&right.device.name)
            .then(left.device.id.0.cmp(&right.device.id.0))
    });
    devices.dedup_by(|later, earlier| later.device.id == earlier.device.id);
    devices
}

fn discovered_of(service: &ResolvedService, me: &DeviceId) -> Option<Discovered> {
    let id = service.get_property_val_str(ID_KEY)?;
    let name = service.get_property_val_str(NAME_KEY).unwrap_or(id);
    let version = service
        .get_property_val_str(VERSION_KEY)?
        .parse::<u16>()
        .ok()?;
    if id == me.0 || version != PROTOCOL_VERSION {
        return None;
    }
    let address = reachable_address(service.get_addresses_v4().into_iter().map(IpAddr::V4))?;
    Some(Discovered {
        device: DeviceInfo {
            id: DeviceId(id.to_owned()),
            name: name.to_owned(),
        },
        address: SocketAddr::new(address, service.get_port()),
    })
}

fn reachable_address(addresses: impl Iterator<Item = IpAddr>) -> Option<IpAddr> {
    addresses
        .filter(|address| is_lan_peer(*address))
        .min_by_key(|address| (address.is_loopback(), *address))
}

fn discovery_error(error: mdns_sd::Error) -> RemoteError {
    RemoteError::Discovery(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn discovered(index: usize) -> Discovered {
        Discovered {
            device: DeviceInfo {
                id: DeviceId(format!("player-{index}")),
                name: format!("Player {index}"),
            },
            address: SocketAddr::new(IpAddr::from([192, 168, 1, 7]), 4000),
        }
    }

    #[test]
    fn given_more_announcements_than_the_cap_only_known_players_are_kept() {
        let mut found = HashMap::new();
        for index in 0..MAX_DISCOVERED_DEVICES {
            assert!(remember(
                &mut found,
                format!("player-{index}"),
                discovered(index)
            ));
        }
        let late = MAX_DISCOVERED_DEVICES;
        assert!(!remember(
            &mut found,
            format!("player-{late}"),
            discovered(late)
        ));
        assert!(remember(
            &mut found,
            "player-0".to_owned(),
            discovered(late)
        ));
        assert_eq!(found.len(), MAX_DISCOVERED_DEVICES);
    }

    #[test]
    fn a_network_address_is_preferred_over_loopback() {
        let addresses =
            ["127.0.0.1", "8.8.8.8", "192.168.1.7"].map(|text| text.parse::<IpAddr>().unwrap());
        assert_eq!(
            reachable_address(addresses.into_iter()),
            Some("192.168.1.7".parse().unwrap())
        );
        let only_loopback = ["127.0.0.1".parse::<IpAddr>().unwrap()];
        assert_eq!(
            reachable_address(only_loopback.into_iter()),
            Some("127.0.0.1".parse().unwrap())
        );
        assert_eq!(
            reachable_address(["8.8.8.8".parse::<IpAddr>().unwrap()].into_iter()),
            None
        );
    }
}
