use std::net::IpAddr;

pub fn is_lan_peer(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(mapped) => is_lan_peer(IpAddr::V4(mapped)),
            None => v6.is_loopback() || v6.is_unique_local() || v6.is_unicast_link_local(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_lan(text: &str) -> bool {
        is_lan_peer(text.parse().unwrap())
    }

    #[test]
    fn only_local_network_addresses_are_accepted() {
        for local in [
            "127.0.0.1",
            "192.168.1.20",
            "10.0.0.5",
            "172.16.3.4",
            "169.254.1.1",
            "::1",
            "fd12::1",
            "fe80::1",
            "::ffff:192.168.0.7",
        ] {
            assert!(is_lan(local), "{local}");
        }
        for public in ["8.8.8.8", "1.1.1.1", "2001:4860::8888", "::ffff:8.8.8.8"] {
            assert!(!is_lan(public), "{public}");
        }
    }
}
