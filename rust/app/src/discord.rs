//! Discord Rich Presence as a media session: mirrors the now playing metadata and status.
//!
//! Connecting blocks, so a background thread owns the connection, retries with backoff while Discord
//! is not running and always delivers the latest state.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use feishin_platform::{DiscordPresence, NowPlaying, PlaybackStatus};

use crate::media::MediaSession;

const FIRST_RETRY: Duration = Duration::from_secs(5);
const MAX_RETRY: Duration = Duration::from_secs(300);

enum Update {
    Metadata(NowPlaying),
    Status(PlaybackStatus, Instant),
}

pub struct DiscordSession {
    updates: Sender<Update>,
}

impl DiscordSession {
    /// An empty `client_id` uses Feishin's application.
    pub fn new(client_id: String) -> Result<Self> {
        let (updates, receiver) = mpsc::channel();
        std::thread::Builder::new()
            .name("discord-presence".into())
            .spawn(move || run(&client_id, &receiver))?;
        Ok(Self { updates })
    }

    fn send(&self, update: Update) -> Result<()> {
        self.updates
            .send(update)
            .map_err(|_| anyhow!("Discord presence thread is gone"))
    }
}

impl MediaSession for DiscordSession {
    fn set_metadata(&mut self, now_playing: &NowPlaying) -> Result<()> {
        self.send(Update::Metadata(now_playing.clone()))
    }
    fn set_status(&mut self, status: PlaybackStatus) -> Result<()> {
        self.send(Update::Status(status, Instant::now()))
    }
    fn set_volume(&mut self, _volume: f64) -> Result<()> {
        Ok(())
    }
}

/// Doubling retry delay, capped.
#[derive(Debug)]
struct Backoff {
    delay: Duration,
    next_attempt: Instant,
}

impl Backoff {
    fn new(now: Instant) -> Self {
        Self {
            delay: FIRST_RETRY,
            next_attempt: now,
        }
    }

    fn failed(&mut self, now: Instant) {
        self.next_attempt = now + self.delay;
        self.delay = (self.delay * 2).min(MAX_RETRY);
    }

    fn reset(&mut self, now: Instant) {
        *self = Self::new(now);
    }
}

#[derive(Default)]
struct Desired {
    now_playing: Option<NowPlaying>,
    status: Option<(PlaybackStatus, Instant)>,
}

impl Desired {
    fn apply(&mut self, update: Update) {
        match update {
            Update::Metadata(now_playing) => self.now_playing = Some(now_playing),
            Update::Status(status, at) => self.status = Some((status, at)),
        }
    }

    /// What to show at `now`: the song, its position and whether it is paused; `None` clears it.
    fn activity(&self, now: Instant) -> Option<(&NowPlaying, u64, bool)> {
        let now_playing = self.now_playing.as_ref()?;
        match self.status? {
            (PlaybackStatus::Playing { position_ms }, at) => {
                let elapsed = now.saturating_duration_since(at).as_millis() as u64;
                Some((now_playing, position_ms.saturating_add(elapsed), false))
            }
            (PlaybackStatus::Paused { position_ms }, _) => Some((now_playing, position_ms, true)),
            (PlaybackStatus::Stopped, _) => None,
        }
    }
}

fn run(client_id: &str, updates: &Receiver<Update>) {
    let mut desired = Desired::default();
    let mut presence: Option<DiscordPresence> = None;
    let mut backoff = Backoff::new(Instant::now());
    let mut dirty = false;
    loop {
        let wants_connection = presence.is_none() && desired.activity(Instant::now()).is_some();
        let received = if wants_connection {
            let wait = backoff
                .next_attempt
                .saturating_duration_since(Instant::now());
            updates.recv_timeout(wait)
        } else {
            updates.recv().map_err(|_| RecvTimeoutError::Disconnected)
        };
        match received {
            Ok(update) => {
                desired.apply(update);
                updates.try_iter().for_each(|update| desired.apply(update));
                dirty = true;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }

        let now = Instant::now();
        if presence.is_none() {
            if desired.activity(now).is_none() || now < backoff.next_attempt {
                continue;
            }
            match DiscordPresence::connect(client_id) {
                Ok(connected) => {
                    presence = Some(connected);
                    backoff.reset(now);
                    dirty = true;
                }
                Err(e) => {
                    tracing::debug!("Discord presence: {e}");
                    backoff.failed(Instant::now());
                    continue;
                }
            }
        }
        let Some(client) = presence.as_mut().filter(|_| dirty) else {
            continue;
        };
        dirty = false;
        let result = match desired.activity(Instant::now()) {
            Some((now_playing, position_ms, paused)) => {
                client.set_activity(now_playing, Some(position_ms), paused)
            }
            None => client.clear(),
        };
        if let Err(e) = result {
            tracing::debug!("Discord presence: {e}");
            presence = None;
            backoff.failed(Instant::now());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_up_to_the_cap() {
        let start = Instant::now();
        let mut backoff = Backoff::new(start);
        assert_eq!(backoff.next_attempt, start);
        backoff.failed(start);
        assert_eq!(backoff.next_attempt, start + FIRST_RETRY);
        for _ in 0..20 {
            backoff.failed(start);
        }
        assert_eq!(backoff.next_attempt, start + MAX_RETRY);
        backoff.reset(start);
        assert_eq!(backoff.delay, FIRST_RETRY);
    }

    #[test]
    fn activity_follows_status_and_advances_while_playing() {
        let start = Instant::now();
        let mut desired = Desired::default();
        desired.apply(Update::Status(
            PlaybackStatus::Playing { position_ms: 1_000 },
            start,
        ));
        assert!(desired.activity(start).is_none());

        let song = NowPlaying {
            track_id: "s1".into(),
            title: "Song".into(),
            ..NowPlaying::default()
        };
        desired.apply(Update::Metadata(song.clone()));
        assert_eq!(
            desired.activity(start + Duration::from_secs(2)),
            Some((&song, 3_000, false))
        );

        desired.apply(Update::Status(
            PlaybackStatus::Paused { position_ms: 4_000 },
            start,
        ));
        assert_eq!(
            desired.activity(start + Duration::from_secs(9)),
            Some((&song, 4_000, true))
        );

        desired.apply(Update::Status(PlaybackStatus::Stopped, start));
        assert!(desired.activity(start).is_none());
    }
}
