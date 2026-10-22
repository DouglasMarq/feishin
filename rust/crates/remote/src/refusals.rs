use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

const COOLDOWN: Duration = Duration::from_secs(120);
const MAX_REMEMBERED_PEERS: usize = 256;

#[derive(Default)]
pub struct Refusals(Mutex<HashMap<IpAddr, Instant>>);

impl Refusals {
    pub fn is_cooling_down(&self, peer: IpAddr, now: Instant) -> bool {
        self.refused()
            .get(&peer)
            .is_some_and(|refused_at| is_recent(*refused_at, now))
    }

    pub fn remember(&self, peer: IpAddr, now: Instant) {
        let mut refused = self.refused();
        refused.retain(|_peer, refused_at| is_recent(*refused_at, now));
        if refused.len() >= MAX_REMEMBERED_PEERS && !refused.contains_key(&peer) {
            let oldest = refused
                .iter()
                .min_by_key(|(_peer, refused_at)| **refused_at)
                .map(|(oldest, _refused_at)| *oldest);
            if let Some(oldest) = oldest {
                refused.remove(&oldest);
            }
        }
        refused.insert(peer, now);
    }

    fn refused(&self) -> MutexGuard<'_, HashMap<IpAddr, Instant>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn is_recent(refused_at: Instant, now: Instant) -> bool {
    now.saturating_duration_since(refused_at) < COOLDOWN
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address(last: u8) -> IpAddr {
        IpAddr::from([192, 168, 0, last])
    }

    #[test]
    fn given_a_refused_address_it_cools_down_until_the_cooldown_ends() {
        let start = Instant::now();
        let refusals = Refusals::default();
        refusals.remember(address(9), start);

        assert!(refusals.is_cooling_down(address(9), start + COOLDOWN - Duration::from_secs(1)));
        assert!(!refusals.is_cooling_down(address(9), start + COOLDOWN));
        assert!(!refusals.is_cooling_down(address(10), start));
    }

    #[test]
    fn given_more_refused_addresses_than_the_cap_the_oldest_is_forgotten() {
        let start = Instant::now();
        let refusals = Refusals::default();
        let peers: Vec<IpAddr> = (0..=MAX_REMEMBERED_PEERS)
            .map(|index| IpAddr::from([10, 0, (index / 256) as u8, (index % 256) as u8]))
            .collect();
        for (offset, peer) in peers.iter().enumerate() {
            refusals.remember(*peer, start + Duration::from_millis(offset as u64));
        }

        let now = start + Duration::from_secs(1);
        assert_eq!(refusals.refused().len(), MAX_REMEMBERED_PEERS);
        assert!(!refusals.is_cooling_down(peers[0], now));
        assert!(refusals.is_cooling_down(peers[MAX_REMEMBERED_PEERS], now));
    }
}
