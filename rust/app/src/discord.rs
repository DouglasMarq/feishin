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

#[derive(Debug, PartialEq)]
struct Activity<'desired> {
    now_playing: &'desired NowPlaying,
    position_ms: u64,
    paused: bool,
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
            Update::Status(status, received_at) => self.status = Some((status, received_at)),
        }
    }

    fn activity(&self, now: Instant) -> Option<Activity<'_>> {
        let now_playing = self.now_playing.as_ref()?;
        match self.status? {
            (PlaybackStatus::Playing { position_ms }, received_at) => {
                let elapsed_ms = now.saturating_duration_since(received_at).as_millis() as u64;
                Some(Activity {
                    now_playing,
                    position_ms: position_ms.saturating_add(elapsed_ms),
                    paused: false,
                })
            }
            (PlaybackStatus::Paused { position_ms }, _) => Some(Activity {
                now_playing,
                position_ms,
                paused: true,
            }),
            (PlaybackStatus::Stopped, _) => None,
        }
    }
}

fn run(client_id: &str, updates: &Receiver<Update>) {
    let mut worker = PresenceWorker::new(client_id);
    while worker.receive(updates) {
        worker.sync();
    }
}

struct PresenceWorker<'client> {
    client_id: &'client str,
    desired: Desired,
    presence: Option<DiscordPresence>,
    backoff: Backoff,
    dirty: bool,
}

impl<'client> PresenceWorker<'client> {
    fn new(client_id: &'client str) -> Self {
        Self {
            client_id,
            desired: Desired::default(),
            presence: None,
            backoff: Backoff::new(Instant::now()),
            dirty: false,
        }
    }

    fn receive(&mut self, updates: &Receiver<Update>) -> bool {
        match self.wait_for_update(updates) {
            Ok(update) => {
                self.desired.apply(update);
                updates
                    .try_iter()
                    .for_each(|update| self.desired.apply(update));
                self.dirty = true;
                true
            }
            Err(RecvTimeoutError::Timeout) => true,
            Err(RecvTimeoutError::Disconnected) => false,
        }
    }

    fn wait_for_update(&self, updates: &Receiver<Update>) -> Result<Update, RecvTimeoutError> {
        let wants_connection =
            self.presence.is_none() && self.desired.activity(Instant::now()).is_some();
        if !wants_connection {
            return updates.recv().map_err(|_| RecvTimeoutError::Disconnected);
        }
        let wait = self
            .backoff
            .next_attempt
            .saturating_duration_since(Instant::now());
        updates.recv_timeout(wait)
    }

    fn sync(&mut self) {
        if self.presence.is_none() && !self.try_connect() {
            return;
        }
        self.push_activity();
    }

    fn try_connect(&mut self) -> bool {
        let now = Instant::now();
        if self.desired.activity(now).is_none() || now < self.backoff.next_attempt {
            return false;
        }
        match DiscordPresence::connect(self.client_id) {
            Ok(connected) => {
                self.presence = Some(connected);
                self.backoff.reset(now);
                self.dirty = true;
                true
            }
            Err(error) => {
                tracing::debug!("Discord presence: {error}");
                self.backoff.failed(Instant::now());
                false
            }
        }
    }

    fn push_activity(&mut self) {
        let Some(client) = self.presence.as_mut().filter(|_| self.dirty) else {
            return;
        };
        self.dirty = false;
        let result = match self.desired.activity(Instant::now()) {
            Some(activity) => client.set_activity(
                activity.now_playing,
                Some(activity.position_ms),
                activity.paused,
            ),
            None => client.clear(),
        };
        if let Err(error) = result {
            tracing::debug!("Discord presence: {error}");
            self.presence = None;
            self.backoff.failed(Instant::now());
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
            Some(Activity {
                now_playing: &song,
                position_ms: 3_000,
                paused: false,
            })
        );

        desired.apply(Update::Status(
            PlaybackStatus::Paused { position_ms: 4_000 },
            start,
        ));
        assert_eq!(
            desired.activity(start + Duration::from_secs(9)),
            Some(Activity {
                now_playing: &song,
                position_ms: 4_000,
                paused: true,
            })
        );

        desired.apply(Update::Status(PlaybackStatus::Stopped, start));
        assert!(desired.activity(start).is_none());
    }
}
