//! Playback state machine: owns the queue and drives the audio output, media session and scrobbling.

use std::sync::Arc;

use feishin_api::{Credential, MusicServer, StreamOptions};
use feishin_core::model::{ServerConfig, ServerPlayQueue, Song, SongListQuery};
use feishin_core::queue::{Advance, Queue, QueueItem, RepeatMode, Uid};
use feishin_platform::{MediaControlEvent, NowPlaying, PlaybackStatus};
use feishin_player::{AudioDevice, PlayerEvent};
use feishin_storage::queue_store::StoredQueue;
use feishin_storage::settings::{IntegrationSettings, PlaybackSettings, Settings, UiSettings};
use tokio::sync::{mpsc, oneshot};

use crate::audio::{AudioOutput, Track};
use crate::library::fetch_all_songs;
use crate::media::MediaSession;
use crate::play_queue_sync::{PlayQueueState, PlayQueueSync};
use crate::scrobble::ScrobbleTracker;
use crate::session::Session;
use crate::sleep_timer::{SleepMode, SleepStatus, SleepTimer};
use crate::store::Store;

/// Going back further than this into a track restarts it instead of moving to the previous one.
const PREVIOUS_RESTARTS_AFTER_MS: u64 = 10_000;

pub enum Command {
    Player(PlayerEvent),
    Media(MediaControlEvent),
    /// A server was connected; it is already set on the session.
    Connected {
        config: ServerConfig,
        others: Vec<ServerConfig>,
    },
    PlayFromList {
        query: SongListQuery,
        song_id: String,
    },
    PlayNow {
        request: u64,
        songs: Vec<Song>,
        song_id: String,
    },
    TogglePause,
    Next,
    Previous,
    Seek(u64),
    SetVolume(u8),
    ToggleMute,
    ToggleShuffle,
    CycleRepeat,
    PlayUid(Uid),
    /// Replace the queue with `songs` already in hand (album, playlist, search results), starting at `song_id`.
    PlaySongs {
        songs: Vec<Song>,
        song_id: Option<String>,
    },
    AddNext(Vec<Song>),
    AddLast(Vec<Song>),
    Remove(Vec<Uid>),
    Move {
        uids: Vec<Uid>,
        before: Option<Uid>,
    },
    /// Starts or replaces the sleep timer; `None` cancels it.
    SleepTimer(Option<SleepMode>),
    SleepTick {
        elapsed_ms: u64,
    },
    RestoreQueue {
        request: u64,
        queue: ServerPlayQueue,
    },
    Shutdown {
        ui: UiSettings,
        done: oneshot::Sender<()>,
    },
    GetSettings(oneshot::Sender<SettingsSnapshot>),
    /// Applies and saves playback settings; volume and mute keep the player's current values.
    ApplyPlayback(PlaybackSettings),
    SaveIntegrations(IntegrationSettings),
    SaveInterface {
        queue_panel_open: bool,
        follow_current_song: bool,
    },
    /// Forgets a saved server and its credential; removing the current one disconnects.
    RemoveServer(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SettingsSnapshot {
    pub settings: Settings,
    pub devices: Vec<AudioDevice>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueueEntry {
    pub uid: Uid,
    pub song_id: String,
    pub title: String,
    pub artists: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UiUpdate {
    NowPlaying(Option<Box<Song>>),
    Playback {
        playing: bool,
        position_ms: u64,
        duration_ms: u64,
    },
    Volume {
        volume: u8,
        muted: bool,
    },
    Modes {
        shuffle: bool,
        repeat: RepeatMode,
    },
    Queue {
        entries: Vec<QueueEntry>,
        current: Option<Uid>,
    },
    QueueCurrent(Option<Uid>),
    Error(String),
    Raise,
    Quit,
}

pub trait UiSink: Send + Sync {
    fn send(&self, update: UiUpdate);
    fn sleep_timer(&self, _status: Option<SleepStatus>) {}
}

pub struct Deps {
    pub audio: Box<dyn AudioOutput>,
    pub media: Box<dyn MediaSession>,
    pub store: Arc<dyn Store>,
    pub session: Session,
    pub ui: Arc<dyn UiSink>,
    pub commands: mpsc::UnboundedSender<Command>,
}

pub struct Controller {
    deps: Deps,
    queue: Queue,
    settings: Settings,
    scrobble: ScrobbleTracker,
    loaded: Option<Uid>,
    loading: Option<Uid>,
    preloaded: Option<Uid>,
    playing: bool,
    position_ms: u64,
    duration_ms: u64,
    play_request: u64,
    queue_sync: PlayQueueSync,
    sleep: SleepTimer,
    saved_credential: Option<Credential>,
}

pub fn artist_names(song: &Song) -> String {
    song.artists
        .iter()
        .map(|artist| artist.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

impl Controller {
    pub fn new(
        deps: Deps,
        settings: Settings,
        stored: Option<StoredQueue>,
        seed: Option<u64>,
    ) -> Self {
        let (queue, position_ms) = match stored {
            Some(stored) => (Queue::from_snapshot(stored.snapshot), stored.position_ms),
            None => (seed.map_or_else(Queue::new, Queue::with_seed), 0),
        };
        let duration_ms = queue.current().map_or(0, |item| item.song.duration_ms);
        let mut controller = Self {
            deps,
            queue,
            settings,
            scrobble: ScrobbleTracker::default(),
            loaded: None,
            loading: None,
            preloaded: None,
            playing: false,
            position_ms,
            duration_ms,
            play_request: 0,
            queue_sync: PlayQueueSync::default(),
            sleep: SleepTimer::default(),
            saved_credential: None,
        };
        controller.publish_all();
        controller.update_media_volume();
        controller
    }

    pub async fn run(mut self, mut commands: mpsc::UnboundedReceiver<Command>) {
        while let Some(command) = commands.recv().await {
            if !self.handle(command) {
                break;
            }
        }
    }

    #[cfg(test)]
    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    #[cfg(test)]
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    #[cfg(test)]
    pub fn sleep_status(&self) -> Option<SleepStatus> {
        self.sleep.status()
    }

    /// Returns `false` once the controller has shut down.
    pub fn handle(&mut self, command: Command) -> bool {
        match command {
            Command::Player(event) => self.on_player_event(event),
            Command::Media(event) => self.on_media_event(event),
            Command::Connected { config, others } => self.connected(config, others),
            Command::PlayFromList { query, song_id } => self.play_from_list(query, song_id),
            Command::PlayNow {
                request,
                songs,
                song_id,
            } if request == self.play_request => self.play_now(songs, &song_id),
            Command::PlayNow { .. } => {}
            Command::TogglePause => self.toggle_pause(),
            Command::Next => self.next(),
            Command::Previous => self.previous(),
            Command::Seek(position_ms) => self.seek(position_ms),
            Command::SetVolume(volume) => self.set_volume(volume),
            Command::ToggleMute => self.set_muted(!self.settings.playback.muted),
            Command::ToggleShuffle => {
                let on = !self.queue.shuffle();
                self.queue.set_shuffle(on);
                self.queue_changed();
            }
            Command::CycleRepeat => {
                self.queue.set_repeat(match self.queue.repeat() {
                    RepeatMode::None => RepeatMode::All,
                    RepeatMode::All => RepeatMode::One,
                    RepeatMode::One => RepeatMode::None,
                });
                self.publish_modes();
                self.refresh_preload();
            }
            Command::PlayUid(uid) => {
                if self.queue.play_uid(uid).is_some() {
                    self.load_current(0);
                }
            }
            Command::PlaySongs { songs, song_id } => {
                self.play_request += 1;
                self.queue.play_now(songs, song_id.as_deref());
                self.publish_queue();
                self.load_current(0);
            }
            Command::AddNext(songs) => {
                self.queue.add_next(songs);
                self.queue_changed();
            }
            Command::AddLast(songs) => {
                self.queue.add_last(songs);
                self.queue_changed();
            }
            Command::Remove(uids) => self.remove(&uids),
            Command::Move { uids, before } => {
                self.queue.move_items(&uids, before);
                self.queue_changed();
            }
            Command::SleepTimer(mode) => self.set_sleep_timer(mode),
            Command::SleepTick { elapsed_ms } => {
                if self.sleep.tick(elapsed_ms, self.playing) && self.playing {
                    self.toggle_pause();
                }
                self.deps.ui.sleep_timer(self.sleep.status());
            }
            Command::RestoreQueue { request, queue } => self.restore_queue(request, queue),
            Command::Shutdown { ui, done } => {
                self.settings.ui = UiSettings {
                    follow_current_song: self.settings.ui.follow_current_song,
                    ..ui
                };
                self.persist();
                self.persist_credential();
                match self.play_queue_state() {
                    Some((server, state)) => {
                        let sync = self.queue_sync.clone();
                        tokio::spawn(async move {
                            sync.flush(server, state).await;
                            let _ = done.send(());
                        });
                    }
                    None => {
                        let _ = done.send(());
                    }
                }
                return false;
            }
            Command::GetSettings(reply) => {
                let devices = self.deps.audio.audio_devices().unwrap_or_else(|e| {
                    tracing::warn!("listing audio devices failed: {e:#}");
                    Vec::new()
                });
                let _ = reply.send(SettingsSnapshot {
                    settings: self.settings.clone(),
                    devices,
                });
            }
            Command::ApplyPlayback(playback) => self.apply_playback(playback),
            Command::SaveIntegrations(integrations) => {
                self.settings.integrations = integrations;
                self.save_settings();
            }
            Command::SaveInterface {
                queue_panel_open,
                follow_current_song,
            } => {
                self.settings.ui.queue_panel_open = queue_panel_open;
                self.settings.ui.follow_current_song = follow_current_song;
                self.save_settings();
            }
            Command::RemoveServer(id) => self.remove_server(&id),
        }
        true
    }

    fn save_settings(&self) {
        if let Err(e) = self.deps.store.save_settings(&self.settings) {
            tracing::warn!("saving settings failed: {e:#}");
        }
    }

    fn apply_playback(&mut self, playback: PlaybackSettings) {
        let current = &self.settings.playback;
        let next = PlaybackSettings {
            volume: current.volume,
            muted: current.muted,
            ..playback
        };
        let audio = &self.deps.audio;
        if next.replay_gain != current.replay_gain
            && let Err(e) = audio.set_replay_gain(next.replay_gain)
        {
            tracing::warn!("setting ReplayGain failed: {e:#}");
        }
        if next.audio_device != current.audio_device
            && let Err(e) = audio.set_audio_device(next.audio_device.as_deref())
        {
            tracing::warn!("switching audio device failed: {e:#}");
        }
        let stale_preload = next.gapless != current.gapless
            || next.max_bitrate_kbps != current.max_bitrate_kbps
            || next.transcode_format != current.transcode_format;
        self.settings.playback = next;
        if stale_preload
            && self.preloaded.take().is_some()
            && let Err(e) = self.deps.audio.clear_next()
        {
            tracing::warn!("clearing the preload failed: {e:#}");
        }
        self.refresh_preload();
        self.save_settings();
    }

    fn remove_server(&mut self, id: &str) {
        if let Err(e) = self.deps.store.delete_credential(id) {
            tracing::warn!("deleting the saved login failed: {e:#}");
        }
        self.settings.servers.retain(|server| server.id != id);
        if self.settings.current_server_id.as_deref() == Some(id) {
            self.settings.current_server_id = None;
            self.deps.session.set(None);
            self.clear_queue();
        }
        self.save_settings();
    }

    fn clear_queue(&mut self) {
        self.queue.clear();
        self.stop();
        self.deps.ui.send(UiUpdate::NowPlaying(None));
        self.queue_changed();
    }

    pub fn persist(&self) {
        if let Err(e) = self.deps.store.save_settings(&self.settings) {
            tracing::warn!("saving settings failed: {e:#}");
        }
        let stored = StoredQueue {
            snapshot: self.queue.snapshot(),
            position_ms: self.position_ms,
        };
        if let Err(e) = self.deps.store.save_queue(&stored) {
            tracing::warn!("saving queue failed: {e:#}");
        }
    }

    fn connected(&mut self, config: ServerConfig, others: Vec<ServerConfig>) {
        if self
            .settings
            .current_server_id
            .as_ref()
            .is_some_and(|id| *id != config.id)
        {
            self.clear_queue();
        }
        let servers = &mut self.settings.servers;
        for server in others.into_iter().chain(std::iter::once(config.clone())) {
            servers.retain(|s| s.id != server.id);
            servers.push(server);
        }
        self.settings.current_server_id = Some(config.id);
        if let Err(e) = self.deps.store.save_settings(&self.settings) {
            tracing::warn!("saving settings failed: {e:#}");
        }
        self.persist_credential();
        if self.queue.is_empty() {
            self.fetch_server_queue();
        } else if let Some(song) = self.queue.current().map(|item| item.song.clone()) {
            self.update_media_metadata(&song);
            self.deps
                .ui
                .send(UiUpdate::NowPlaying(Some(Box::new(song))));
        }
    }

    fn persist_credential(&mut self) {
        let Some(server) = self.deps.session.get() else {
            return;
        };
        let Some(credential) = server.credential() else {
            return;
        };
        if self.saved_credential.as_ref() == Some(&credential) {
            return;
        }
        match self
            .deps
            .store
            .set_credential(&server.config().id, &credential)
        {
            Ok(()) => self.saved_credential = Some(credential),
            Err(e) => tracing::warn!("saving credential failed: {e:#}"),
        }
    }

    fn fetch_server_queue(&self) {
        let Some(server) = self.deps.session.get() else {
            return;
        };
        let request = self.play_request;
        let commands = self.deps.commands.clone();
        tokio::spawn(async move {
            match server.play_queue().await {
                Ok(Some(queue)) if !queue.songs.is_empty() => {
                    let _ = commands.send(Command::RestoreQueue { request, queue });
                }
                Ok(_) => {}
                Err(e) => tracing::debug!("server play queue: {e}"),
            }
        });
    }

    fn restore_queue(&mut self, request: u64, queue: ServerPlayQueue) {
        if request != self.play_request || !self.queue.is_empty() {
            return;
        }
        self.queue.set_shuffle(false);
        self.queue
            .play_now(queue.songs, queue.current_song_id.as_deref());
        self.position_ms = queue.position_ms;
        self.duration_ms = self.queue.current().map_or(0, |item| item.song.duration_ms);
        if let Some((_, state)) = self.play_queue_state() {
            self.queue_sync.mark_saved(state);
        }
        self.publish_all();
    }

    fn play_queue_state(&self) -> Option<(Arc<dyn MusicServer>, PlayQueueState)> {
        let server = self.deps.session.get()?;
        let state = PlayQueueState::of(&self.queue, self.position_ms, &server.config().id)?;
        Some((server, state))
    }

    fn schedule_queue_sync(&self) {
        if let Some((server, state)) = self.play_queue_state() {
            self.queue_sync.schedule(server, state);
        }
    }

    fn set_sleep_timer(&mut self, mode: Option<SleepMode>) {
        match mode {
            Some(mode) => self.sleep.start(mode, &self.deps.commands),
            None => self.sleep.cancel(),
        }
        self.deps.ui.sleep_timer(self.sleep.status());
        self.refresh_preload();
    }

    fn play_from_list(&mut self, query: SongListQuery, song_id: String) {
        self.play_request += 1;
        let request = self.play_request;
        let Some(server) = self.deps.session.get() else {
            return;
        };
        let commands = self.deps.commands.clone();
        let ui = Arc::clone(&self.deps.ui);
        tokio::spawn(async move {
            match fetch_all_songs(server.as_ref(), &query).await {
                Ok(songs) => {
                    let _ = commands.send(Command::PlayNow {
                        request,
                        songs,
                        song_id,
                    });
                }
                Err(e) => ui.send(UiUpdate::Error(format!("Could not load the list: {e}"))),
            }
        });
    }

    pub fn play_now(&mut self, songs: Vec<Song>, song_id: &str) {
        self.queue.play_now(songs, Some(song_id));
        self.publish_queue();
        self.load_current(0);
    }

    fn track(&self, item: &QueueItem) -> Option<Track> {
        let server = self.deps.session.get()?;
        let playback = &self.settings.playback;
        let options = StreamOptions {
            max_bitrate_kbps: playback.max_bitrate_kbps,
            format: playback.transcode_format.clone(),
        };
        Some(Track {
            uid: item.uid,
            url: server.stream_url(&item.song, &options),
            duration_ms: item.song.duration_ms,
        })
    }

    fn load_current(&mut self, start_ms: u64) {
        let Some(item) = self.queue.current() else {
            self.stop();
            return;
        };
        let Some(track) = self.track(item) else {
            self.deps
                .ui
                .send(UiUpdate::Error("Not connected to a server".into()));
            return;
        };
        let song = item.song.clone();
        self.scrobble.stop();
        self.preloaded = None;
        self.position_ms = start_ms;
        self.duration_ms = song.duration_ms;
        match self.deps.audio.load(&track, start_ms) {
            Ok(()) => {
                self.loaded = Some(track.uid);
                self.loading = Some(track.uid);
                self.playing = true;
            }
            Err(e) => {
                self.loaded = None;
                self.loading = None;
                self.playing = false;
                self.deps
                    .ui
                    .send(UiUpdate::Error(format!("Playback failed: {e:#}")));
            }
        }
        self.deps
            .ui
            .send(UiUpdate::NowPlaying(Some(Box::new(song))));
        self.deps.ui.send(UiUpdate::QueueCurrent(Some(track.uid)));
        self.publish_playback();
        self.schedule_queue_sync();
    }

    fn stop(&mut self) {
        if let Err(e) = self.deps.audio.stop() {
            tracing::warn!("stop failed: {e:#}");
        }
        self.loaded = None;
        self.loading = None;
        self.preloaded = None;
        self.playing = false;
        self.position_ms = 0;
        self.scrobble.stop();
        self.update_media_status();
        self.publish_playback();
    }

    fn on_player_event(&mut self, event: PlayerEvent) {
        match event {
            PlayerEvent::Started { uid } => self.on_started(uid),
            PlayerEvent::Position {
                position_ms,
                duration_ms,
            } => {
                if self.loading.is_some() || self.loaded.is_none() {
                    return;
                }
                self.position_ms = position_ms;
                if let Some(duration_ms) = duration_ms.filter(|d| *d > 0) {
                    self.duration_ms = duration_ms;
                }
                if let Some(submission) = self.scrobble.position(position_ms, self.playing) {
                    self.send_scrobble(submission.song_id, true, Some(submission.timestamp_ms));
                }
                self.publish_playback();
            }
            PlayerEvent::Paused => self.set_playing(false),
            PlayerEvent::Resumed => self.set_playing(true),
            PlayerEvent::TrackEnded { uid } => {
                if self.loaded != Some(uid) || self.preloaded.is_some() {
                    return;
                }
                if self.sleep.ends_with_track() {
                    self.sleep_at_track_end();
                } else if self.queue.next(Advance::Auto).is_some() {
                    self.load_current(0);
                } else {
                    self.stop();
                }
            }
            PlayerEvent::Error { uid, message } => {
                if uid.is_none() || uid == self.loaded {
                    self.set_playing(false);
                }
                self.deps.ui.send(UiUpdate::Error(message));
            }
            PlayerEvent::Idle => {
                if self.loading.is_none() && self.loaded.is_some() {
                    self.preloaded = None;
                    self.set_playing(false);
                }
            }
        }
    }

    fn on_started(&mut self, uid: Uid) {
        if self.loading == Some(uid) {
            self.loading = None;
        } else if self.preloaded == Some(uid) {
            self.preloaded = None;
            if self.queue.next(Advance::Auto).map(|item| item.uid) != Some(uid) {
                self.queue.play_uid(uid);
            }
            self.loaded = Some(uid);
            self.position_ms = 0;
            self.duration_ms = self.queue.current().map_or(0, |item| item.song.duration_ms);
            let song = self.queue.current().map(|item| Box::new(item.song.clone()));
            self.deps.ui.send(UiUpdate::NowPlaying(song));
            self.publish_queue_current();
            self.schedule_queue_sync();
        } else {
            return;
        }
        self.playing = true;
        if let Some(song) = self.queue.current().map(|item| item.song.clone()) {
            self.scrobble.start(&song.id, song.duration_ms, now_ms());
            self.send_scrobble(song.id.clone(), false, None);
            self.update_media_metadata(&song);
        }
        self.update_media_status();
        self.publish_playback();
        self.refresh_preload();
    }

    fn sleep_at_track_end(&mut self) {
        self.sleep.cancel();
        self.deps.ui.sleep_timer(None);
        self.queue.next(Advance::Auto);
        self.stop();
        let song = self.queue.current().map(|item| Box::new(item.song.clone()));
        self.deps.ui.send(UiUpdate::NowPlaying(song));
        self.publish_queue_current();
        self.schedule_queue_sync();
    }

    fn refresh_preload(&mut self) {
        if !self.settings.playback.gapless || self.loaded.is_none() {
            return;
        }
        let next = self
            .queue
            .peek_next()
            .filter(|_| !self.sleep.ends_with_track())
            .and_then(|item| self.track(item));
        if next.as_ref().map(|track| track.uid) == self.preloaded {
            return;
        }
        let result = match &next {
            Some(track) => self.deps.audio.preload_next(track),
            None => self.deps.audio.clear_next(),
        };
        match result {
            Ok(()) => self.preloaded = next.map(|track| track.uid),
            Err(e) => tracing::warn!("preloading failed: {e:#}"),
        }
    }

    fn send_scrobble(&self, song_id: String, submission: bool, timestamp_ms: Option<i64>) {
        if !self.settings.playback.scrobble {
            return;
        }
        let Some(server) = self.deps.session.get() else {
            return;
        };
        tokio::spawn(async move {
            if let Err(e) = server.scrobble(&song_id, submission, timestamp_ms).await {
                tracing::warn!("scrobble failed: {e}");
            }
        });
    }

    fn toggle_pause(&mut self) {
        if self.loaded.is_none() {
            if self.queue.current().is_some() {
                self.load_current(self.position_ms);
            }
            return;
        }
        let result = if self.playing {
            self.deps.audio.pause()
        } else {
            self.deps.audio.play()
        };
        match result {
            Ok(()) => self.set_playing(!self.playing),
            Err(e) => tracing::warn!("toggling playback failed: {e:#}"),
        }
    }

    fn set_playing(&mut self, playing: bool) {
        if self.playing != playing {
            self.playing = playing;
            self.update_media_status();
            self.publish_playback();
            if !playing {
                self.schedule_queue_sync();
            }
        }
    }

    fn next(&mut self) {
        if self.queue.next(Advance::Manual).is_some() {
            self.load_current(0);
        }
    }

    fn previous(&mut self) {
        if self.position_ms > PREVIOUS_RESTARTS_AFTER_MS {
            self.seek(0);
            return;
        }
        let current = self.queue.current().map(|item| item.uid);
        match self.queue.previous().map(|item| item.uid) {
            Some(uid) if Some(uid) != current => self.load_current(0),
            _ => self.seek(0),
        }
    }

    fn seek(&mut self, position_ms: u64) {
        if self.loaded.is_none() {
            self.position_ms = position_ms;
        } else if let Err(e) = self.deps.audio.seek(position_ms) {
            tracing::warn!("seek failed: {e:#}");
            return;
        }
        self.position_ms = position_ms;
        self.update_media_status();
        self.publish_playback();
        self.schedule_queue_sync();
    }

    fn set_volume(&mut self, volume: u8) {
        let volume = volume.min(100);
        self.settings.playback.volume = volume;
        if let Err(e) = self.deps.audio.set_volume(volume) {
            tracing::warn!("setting volume failed: {e:#}");
        }
        if volume > 0 && self.settings.playback.muted {
            self.set_muted(false);
        } else {
            self.update_media_volume();
            self.publish_volume();
        }
    }

    fn set_muted(&mut self, muted: bool) {
        self.settings.playback.muted = muted;
        if let Err(e) = self.deps.audio.set_muted(muted) {
            tracing::warn!("muting failed: {e:#}");
        }
        self.update_media_volume();
        self.publish_volume();
    }

    fn update_media_volume(&mut self) {
        let playback = &self.settings.playback;
        let volume = if playback.muted { 0 } else { playback.volume };
        if let Err(e) = self.deps.media.set_volume(f64::from(volume) / 100.0) {
            tracing::debug!("media session volume: {e:#}");
        }
    }

    fn remove(&mut self, uids: &[Uid]) {
        let loaded_removed = self.loaded.is_some_and(|uid| uids.contains(&uid));
        self.queue.remove(uids);
        if self.queue.is_empty() {
            self.stop();
            self.deps.ui.send(UiUpdate::NowPlaying(None));
        } else if loaded_removed {
            if self.playing {
                self.load_current(0);
            } else {
                self.stop();
                let song = self.queue.current().map(|item| Box::new(item.song.clone()));
                self.deps.ui.send(UiUpdate::NowPlaying(song));
            }
        }
        self.queue_changed();
    }

    fn queue_changed(&mut self) {
        self.publish_queue();
        self.publish_modes();
        self.refresh_preload();
        self.schedule_queue_sync();
    }

    fn on_media_event(&mut self, event: MediaControlEvent) {
        match event {
            MediaControlEvent::Play if !self.playing => self.toggle_pause(),
            MediaControlEvent::Pause if self.playing => self.toggle_pause(),
            MediaControlEvent::Play | MediaControlEvent::Pause => {}
            MediaControlEvent::Toggle => self.toggle_pause(),
            MediaControlEvent::Next => self.next(),
            MediaControlEvent::Previous => self.previous(),
            MediaControlEvent::Stop => {
                if self.playing {
                    self.toggle_pause();
                }
                self.seek(0);
            }
            MediaControlEvent::SeekTo { position_ms } => self.seek(position_ms),
            MediaControlEvent::SeekBy { offset_ms } => {
                let target = self.position_ms.saturating_add_signed(offset_ms);
                self.seek(target.min(self.duration_ms));
            }
            MediaControlEvent::SetVolume(volume) => {
                self.set_volume((volume.clamp(0.0, 1.0) * 100.0).round() as u8)
            }
            MediaControlEvent::Raise => self.deps.ui.send(UiUpdate::Raise),
            MediaControlEvent::Quit => self.deps.ui.send(UiUpdate::Quit),
        }
    }

    fn update_media_metadata(&mut self, song: &Song) {
        let cover_url = self.deps.session.get().and_then(|server| {
            song.cover_art_id
                .as_deref()
                .map(|id| server.cover_art_url(id, Some(512)))
        });
        let now_playing = NowPlaying {
            track_id: song.id.clone(),
            title: song.title.clone(),
            artists: song.artists.iter().map(|a| a.name.clone()).collect(),
            album: song.album.clone(),
            duration_ms: Some(song.duration_ms),
            cover_url,
        };
        if let Err(e) = self.deps.media.set_metadata(&now_playing) {
            tracing::debug!("media session metadata: {e:#}");
        }
    }

    fn update_media_status(&mut self) {
        let status = match (self.loaded, self.playing) {
            (None, _) => PlaybackStatus::Stopped,
            (Some(_), true) => PlaybackStatus::Playing {
                position_ms: self.position_ms,
            },
            (Some(_), false) => PlaybackStatus::Paused {
                position_ms: self.position_ms,
            },
        };
        if let Err(e) = self.deps.media.set_status(status) {
            tracing::debug!("media session status: {e:#}");
        }
    }

    fn publish_all(&self) {
        self.publish_volume();
        self.publish_modes();
        self.publish_queue();
        let song = self.queue.current().map(|item| Box::new(item.song.clone()));
        self.deps.ui.send(UiUpdate::NowPlaying(song));
        self.publish_playback();
    }

    fn publish_playback(&self) {
        self.deps.ui.send(UiUpdate::Playback {
            playing: self.playing,
            position_ms: self.position_ms,
            duration_ms: self.duration_ms,
        });
    }

    fn publish_volume(&self) {
        self.deps.ui.send(UiUpdate::Volume {
            volume: self.settings.playback.volume,
            muted: self.settings.playback.muted,
        });
    }

    fn publish_modes(&self) {
        self.deps.ui.send(UiUpdate::Modes {
            shuffle: self.queue.shuffle(),
            repeat: self.queue.repeat(),
        });
    }

    fn publish_queue(&self) {
        let entries = self
            .queue
            .display_order()
            .into_iter()
            .map(|item| QueueEntry {
                uid: item.uid,
                song_id: item.song.id.clone(),
                title: item.song.title.clone(),
                artists: artist_names(&item.song),
                duration_ms: item.song.duration_ms,
            })
            .collect();
        self.deps.ui.send(UiUpdate::Queue {
            entries,
            current: self.queue.current().map(|item| item.uid),
        });
    }

    fn publish_queue_current(&self) {
        self.deps.ui.send(UiUpdate::QueueCurrent(
            self.queue.current().map(|item| item.uid),
        ));
    }
}
