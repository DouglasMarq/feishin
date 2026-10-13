use std::sync::{Arc, Mutex, MutexGuard, PoisonError, mpsc as std_mpsc};
use std::thread;
use std::time::{Duration, Instant};

use mpris_server::zbus::{self, fdo};
use mpris_server::{
    LoopStatus, Metadata, PlaybackRate, PlaybackStatus as MprisStatus, PlayerInterface, Property,
    RootInterface, Server, Signal, Time, TrackId, Volume,
};
use tokio::sync::mpsc;

use crate::{MediaControlEvent, NowPlaying, PlatformError, PlaybackStatus, Result};

const TRACK_PATH_PREFIX: &str = "/org/feishin/track/";
const STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
const SEEK_DETECT_THRESHOLD_MS: u64 = 1000;

type EventCallback = Box<dyn Fn(MediaControlEvent) + Send + Sync>;

enum Update {
    Metadata(NowPlaying),
    Status(PlaybackStatus),
    Volume(f64),
    Emit(Vec<Property>),
}

/// MPRIS on Linux.
///
/// The D-Bus service runs on its own thread with its own runtime; every setter
/// only queues an update and never blocks.
pub struct MediaControls {
    updates: mpsc::UnboundedSender<Update>,
}

impl MediaControls {
    /// Registers `org.mpris.MediaPlayer2.<app_name in lowercase>` on the session bus,
    /// falling back to `<name>.instance<pid>` when the name is taken.
    /// `on_event` is invoked from the MPRIS thread and should return quickly.
    pub fn new(
        app_name: &str,
        on_event: Box<dyn Fn(MediaControlEvent) + Send + Sync>,
    ) -> Result<Self> {
        let (updates, rx) = mpsc::unbounded_channel();
        let (ready_tx, ready_rx) = std_mpsc::sync_channel(1);
        let player = MprisPlayer(Arc::new(Inner {
            identity: app_name.to_owned(),
            bus_suffix: bus_suffix(app_name),
            state: Mutex::new(State::new()),
            on_event,
            updates: updates.downgrade(),
        }));

        thread::Builder::new()
            .name("mpris".to_owned())
            .spawn(move || run(player, rx, ready_tx))
            .map_err(|err| PlatformError::MediaControls(err.to_string()))?;

        match ready_rx.recv_timeout(STARTUP_TIMEOUT) {
            Ok(Ok(())) => Ok(Self { updates }),
            Ok(Err(err)) => Err(err),
            Err(_) => Err(PlatformError::MediaControls(
                "timed out registering on the session bus".to_owned(),
            )),
        }
    }

    pub fn set_metadata(&mut self, now_playing: &NowPlaying) -> Result<()> {
        self.send(Update::Metadata(now_playing.clone()))
    }

    pub fn set_status(&mut self, status: PlaybackStatus) -> Result<()> {
        self.send(Update::Status(status))
    }

    /// 0.0-1.0.
    pub fn set_volume(&mut self, volume: f64) -> Result<()> {
        self.send(Update::Volume(volume))
    }

    fn send(&self, update: Update) -> Result<()> {
        self.updates
            .send(update)
            .map_err(|_| PlatformError::MediaControls("MPRIS thread is not running".to_owned()))
    }
}

fn run(
    player: MprisPlayer,
    rx: mpsc::UnboundedReceiver<Update>,
    ready: std_mpsc::SyncSender<Result<()>>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            let _ = ready.send(Err(PlatformError::MediaControls(err.to_string())));
            return;
        }
    };
    runtime.block_on(serve(player, rx, ready));
}

async fn serve(
    player: MprisPlayer,
    mut rx: mpsc::UnboundedReceiver<Update>,
    ready: std_mpsc::SyncSender<Result<()>>,
) {
    let server = match register(player).await {
        Ok(server) => server,
        Err(err) => {
            let _ = ready.send(Err(err));
            return;
        }
    };
    let _ = ready.send(Ok(()));

    while let Some(update) = rx.recv().await {
        let (properties, seeked) = server.imp().0.apply(update, Instant::now());
        if !properties.is_empty()
            && let Err(err) = server.properties_changed(properties).await
        {
            tracing::warn!(%err, "MPRIS properties changed signal failed");
        }
        if let Some(position) = seeked
            && let Err(err) = server.emit(Signal::Seeked { position }).await
        {
            tracing::warn!(%err, "MPRIS seeked signal failed");
        }
    }
}

async fn register(player: MprisPlayer) -> Result<Server<MprisPlayer>> {
    let suffix = player.0.bus_suffix.clone();
    match Server::new(&suffix, player.clone()).await {
        Ok(server) => Ok(server),
        Err(first) => {
            let fallback = format!("{suffix}.instance{}", std::process::id());
            tracing::debug!(%first, %fallback, "MPRIS bus name unavailable, using fallback");
            Server::new(&fallback, player)
                .await
                .map_err(|err| PlatformError::MediaControls(err.to_string()))
        }
    }
}

struct State {
    metadata: Metadata,
    status: MprisStatus,
    position_ms: u64,
    position_at: Instant,
    volume: f64,
}

impl State {
    fn new() -> Self {
        Self {
            metadata: Metadata::new(),
            status: MprisStatus::Stopped,
            position_ms: 0,
            position_at: Instant::now(),
            volume: 1.0,
        }
    }

    fn has_track(&self) -> bool {
        self.metadata.trackid().is_some()
    }

    fn position_ms(&self, now: Instant) -> u64 {
        match self.status {
            MprisStatus::Playing => {
                let elapsed = now.saturating_duration_since(self.position_at).as_millis();
                self.position_ms
                    .saturating_add(u64::try_from(elapsed).unwrap_or(u64::MAX))
            }
            MprisStatus::Paused | MprisStatus::Stopped => self.position_ms,
        }
    }

    fn set_status(&mut self, status: MprisStatus, position_ms: u64, now: Instant) {
        self.status = status;
        self.position_ms = position_ms;
        self.position_at = now;
    }
}

struct Inner {
    identity: String,
    bus_suffix: String,
    state: Mutex<State>,
    on_event: EventCallback,
    updates: mpsc::WeakUnboundedSender<Update>,
}

impl Inner {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn apply(&self, update: Update, now: Instant) -> (Vec<Property>, Option<Time>) {
        let mut state = self.state();
        match update {
            Update::Metadata(now_playing) => {
                let metadata = metadata_for(&now_playing);
                state.metadata = metadata.clone();
                (vec![Property::Metadata(metadata)], None)
            }
            Update::Status(status) => {
                let (status, position_ms) = split_status(status);
                let expected = state.position_ms(now);
                let seeked = status != MprisStatus::Stopped
                    && expected.abs_diff(position_ms) > SEEK_DETECT_THRESHOLD_MS;
                let changed = state.status != status;
                state.set_status(status, position_ms, now);
                let properties = if changed {
                    vec![Property::PlaybackStatus(status)]
                } else {
                    Vec::new()
                };
                (properties, seeked.then(|| time_from_ms(position_ms)))
            }
            Update::Volume(volume) => {
                let volume = volume.clamp(0.0, 1.0);
                if (state.volume - volume).abs() < f64::EPSILON {
                    return (Vec::new(), None);
                }
                state.volume = volume;
                (vec![Property::Volume(volume)], None)
            }
            Update::Emit(properties) => (properties, None),
        }
    }

    fn emit(&self, event: MediaControlEvent) {
        (self.on_event)(event);
    }

    fn notify(&self, properties: Vec<Property>) {
        if let Some(updates) = self.updates.upgrade() {
            let _ = updates.send(Update::Emit(properties));
        }
    }

    fn has_track(&self) -> bool {
        self.state().has_track()
    }

    fn update_status(&self, next: impl FnOnce(MprisStatus) -> MprisStatus) {
        let now = Instant::now();
        let mut state = self.state();
        let status = next(state.status);
        if status == state.status {
            return;
        }
        let position_ms = state.position_ms(now);
        state.set_status(status, position_ms, now);
        drop(state);
        self.notify(vec![Property::PlaybackStatus(status)]);
    }

    fn transport(&self, event: MediaControlEvent, next: impl FnOnce(MprisStatus) -> MprisStatus) {
        if !self.has_track() {
            return;
        }
        self.emit(event);
        self.update_status(next);
    }
}

#[derive(Clone)]
struct MprisPlayer(Arc<Inner>);

fn not_supported(what: &str) -> fdo::Error {
    fdo::Error::NotSupported(format!("{what} is not supported"))
}

impl RootInterface for MprisPlayer {
    async fn raise(&self) -> fdo::Result<()> {
        self.0.emit(MediaControlEvent::Raise);
        Ok(())
    }

    async fn quit(&self) -> fdo::Result<()> {
        self.0.emit(MediaControlEvent::Quit);
        Ok(())
    }

    async fn can_quit(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_fullscreen(&self, _fullscreen: bool) -> zbus::Result<()> {
        Err(not_supported("Fullscreen").into())
    }

    async fn can_set_fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_raise(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn has_track_list(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn identity(&self) -> fdo::Result<String> {
        Ok(self.0.identity.clone())
    }

    async fn desktop_entry(&self) -> fdo::Result<String> {
        Ok(self.0.bus_suffix.clone())
    }

    async fn supported_uri_schemes(&self) -> fdo::Result<Vec<String>> {
        Ok(vec!["file".to_owned()])
    }

    async fn supported_mime_types(&self) -> fdo::Result<Vec<String>> {
        Ok(vec!["audio/mpeg".to_owned(), "application/ogg".to_owned()])
    }
}

impl PlayerInterface for MprisPlayer {
    async fn next(&self) -> fdo::Result<()> {
        self.0
            .transport(MediaControlEvent::Next, |_| MprisStatus::Playing);
        Ok(())
    }

    async fn previous(&self) -> fdo::Result<()> {
        self.0
            .transport(MediaControlEvent::Previous, |_| MprisStatus::Playing);
        Ok(())
    }

    async fn pause(&self) -> fdo::Result<()> {
        self.0
            .transport(MediaControlEvent::Pause, |_| MprisStatus::Paused);
        Ok(())
    }

    async fn play_pause(&self) -> fdo::Result<()> {
        self.0.transport(MediaControlEvent::Toggle, |status| {
            if status == MprisStatus::Playing {
                MprisStatus::Paused
            } else {
                MprisStatus::Playing
            }
        });
        Ok(())
    }

    async fn stop(&self) -> fdo::Result<()> {
        self.0.emit(MediaControlEvent::Stop);
        self.0.update_status(|_| MprisStatus::Paused);
        Ok(())
    }

    async fn play(&self) -> fdo::Result<()> {
        self.0
            .transport(MediaControlEvent::Play, |_| MprisStatus::Playing);
        Ok(())
    }

    async fn seek(&self, offset: Time) -> fdo::Result<()> {
        self.0.emit(MediaControlEvent::SeekBy {
            offset_ms: offset.as_millis(),
        });
        Ok(())
    }

    async fn set_position(&self, _track_id: TrackId, position: Time) -> fdo::Result<()> {
        if let Ok(position_ms) = u64::try_from(position.as_millis()) {
            self.0.emit(MediaControlEvent::SeekTo { position_ms });
        }
        Ok(())
    }

    async fn open_uri(&self, _uri: String) -> fdo::Result<()> {
        Err(not_supported("OpenUri"))
    }

    async fn playback_status(&self) -> fdo::Result<MprisStatus> {
        Ok(self.0.state().status)
    }

    async fn loop_status(&self) -> fdo::Result<LoopStatus> {
        Ok(LoopStatus::None)
    }

    async fn set_loop_status(&self, _loop_status: LoopStatus) -> zbus::Result<()> {
        Err(not_supported("LoopStatus").into())
    }

    async fn rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn set_rate(&self, _rate: PlaybackRate) -> zbus::Result<()> {
        Err(not_supported("Rate").into())
    }

    async fn shuffle(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_shuffle(&self, _shuffle: bool) -> zbus::Result<()> {
        Err(not_supported("Shuffle").into())
    }

    async fn metadata(&self) -> fdo::Result<Metadata> {
        Ok(self.0.state().metadata.clone())
    }

    async fn volume(&self) -> fdo::Result<Volume> {
        Ok(self.0.state().volume)
    }

    async fn set_volume(&self, volume: Volume) -> zbus::Result<()> {
        let volume = volume_from_mpris(volume);
        self.0.state().volume = volume;
        self.0.notify(vec![Property::Volume(volume)]);
        self.0.emit(MediaControlEvent::SetVolume(volume));
        Ok(())
    }

    async fn position(&self) -> fdo::Result<Time> {
        Ok(time_from_ms(self.0.state().position_ms(Instant::now())))
    }

    async fn minimum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn maximum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(1.0)
    }

    async fn can_go_next(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_go_previous(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_play(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_pause(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_seek(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_control(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}

fn time_from_ms(ms: u64) -> Time {
    Time::from_millis(i64::try_from(ms).unwrap_or(i64::MAX))
}

fn split_status(status: PlaybackStatus) -> (MprisStatus, u64) {
    match status {
        PlaybackStatus::Playing { position_ms } => (MprisStatus::Playing, position_ms),
        PlaybackStatus::Paused { position_ms } => (MprisStatus::Paused, position_ms),
        PlaybackStatus::Stopped => (MprisStatus::Stopped, 0),
    }
}

fn volume_from_mpris(volume: f64) -> f64 {
    if volume.is_nan() {
        return 0.0;
    }
    (volume * 100.0).round().clamp(0.0, 100.0) / 100.0
}

fn bus_suffix(app_name: &str) -> String {
    let mut suffix: String = app_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    if suffix.is_empty() || suffix.starts_with(|c: char| c.is_ascii_digit()) {
        suffix.insert(0, '_');
    }
    suffix
}

fn track_id_for(id: &str) -> Option<TrackId> {
    if id.is_empty() {
        return None;
    }
    let element: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    TrackId::try_from(format!("{TRACK_PATH_PREFIX}{element}")).ok()
}

fn non_empty(value: Option<&String>) -> Option<String> {
    value.filter(|v| !v.is_empty()).cloned()
}

fn metadata_for(now_playing: &NowPlaying) -> Metadata {
    let mut metadata = Metadata::new();
    let Some(track_id) = track_id_for(&now_playing.track_id) else {
        return metadata;
    };
    metadata.set_trackid(Some(track_id));
    metadata.set_length(
        now_playing
            .duration_ms
            .filter(|ms| *ms > 0)
            .map(time_from_ms),
    );
    metadata.set_art_url(non_empty(now_playing.cover_url.as_ref()));
    metadata.set_album(non_empty(now_playing.album.as_ref()));
    metadata.set_artist((!now_playing.artists.is_empty()).then(|| now_playing.artists.clone()));
    metadata.set_title(non_empty(Some(&now_playing.title)));
    metadata
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song() -> NowPlaying {
        NowPlaying {
            track_id: "a1b2-c3d4.e5".to_owned(),
            title: "Song".to_owned(),
            artists: vec!["Artist A".to_owned(), "Artist B".to_owned()],
            album: Some("Album".to_owned()),
            duration_ms: Some(215_500),
            cover_url: Some("https://example.com/cover.jpg".to_owned()),
        }
    }

    type Channel = (
        mpsc::UnboundedSender<Update>,
        mpsc::UnboundedReceiver<Update>,
    );

    fn inner(on_event: EventCallback) -> (Inner, Channel) {
        let (tx, rx) = mpsc::unbounded_channel();
        let inner = Inner {
            identity: "Feishin".to_owned(),
            bus_suffix: "feishin".to_owned(),
            state: Mutex::new(State::new()),
            on_event,
            updates: tx.downgrade(),
        };
        (inner, (tx, rx))
    }

    #[test]
    fn metadata_maps_all_fields() {
        let metadata = metadata_for(&song());
        assert_eq!(
            metadata.trackid().map(|id| id.to_string()),
            Some("/org/feishin/track/a1b2_c3d4_e5".to_owned())
        );
        assert_eq!(metadata.length(), Some(Time::from_millis(215_500)));
        assert_eq!(
            metadata.art_url(),
            Some("https://example.com/cover.jpg".to_owned())
        );
        assert_eq!(metadata.album(), Some("Album"));
        assert_eq!(
            metadata.artist(),
            Some(vec!["Artist A".to_owned(), "Artist B".to_owned()])
        );
        assert_eq!(metadata.title(), Some("Song"));
    }

    #[test]
    fn metadata_omits_empty_fields() {
        let metadata = metadata_for(&NowPlaying {
            track_id: "id".to_owned(),
            duration_ms: Some(0),
            album: Some(String::new()),
            ..NowPlaying::default()
        });
        assert!(metadata.trackid().is_some());
        assert_eq!(metadata.length(), None);
        assert_eq!(metadata.art_url(), None);
        assert_eq!(metadata.album(), None);
        assert_eq!(metadata.artist(), None);
        assert_eq!(metadata.title(), None);
    }

    #[test]
    fn metadata_without_track_id_is_empty() {
        let metadata = metadata_for(&NowPlaying {
            title: "Song".to_owned(),
            ..NowPlaying::default()
        });
        assert!(metadata.trackid().is_none());
        assert_eq!(metadata.title(), None);
    }

    #[test]
    fn status_mapping() {
        assert_eq!(
            split_status(PlaybackStatus::Playing { position_ms: 5 }),
            (MprisStatus::Playing, 5)
        );
        assert_eq!(
            split_status(PlaybackStatus::Paused { position_ms: 7 }),
            (MprisStatus::Paused, 7)
        );
        assert_eq!(
            split_status(PlaybackStatus::Stopped),
            (MprisStatus::Stopped, 0)
        );
    }

    #[test]
    fn volume_is_rounded_and_clamped() {
        assert_eq!(volume_from_mpris(0.456), 0.46);
        assert_eq!(volume_from_mpris(1.7), 1.0);
        assert_eq!(volume_from_mpris(-0.2), 0.0);
        assert_eq!(volume_from_mpris(f64::NAN), 0.0);
    }

    #[test]
    fn bus_suffix_is_valid() {
        assert_eq!(bus_suffix("Feishin"), "feishin");
        assert_eq!(bus_suffix("My Player.2"), "my_player_2");
        assert_eq!(bus_suffix("9lives"), "_9lives");
        assert_eq!(bus_suffix(""), "_");
    }

    #[test]
    fn position_advances_only_while_playing() {
        let start = Instant::now();
        let mut state = State::new();
        state.set_status(MprisStatus::Playing, 1_000, start);
        assert_eq!(state.position_ms(start + Duration::from_millis(500)), 1_500);
        state.set_status(MprisStatus::Paused, 2_000, start);
        assert_eq!(state.position_ms(start + Duration::from_secs(10)), 2_000);
    }

    #[test]
    fn status_update_emits_changes_and_seeks() {
        let (inner, _channel) = inner(Box::new(|_| {}));
        let now = Instant::now();

        let (props, seeked) = inner.apply(
            Update::Status(PlaybackStatus::Playing { position_ms: 0 }),
            now,
        );
        assert!(matches!(
            props.as_slice(),
            [Property::PlaybackStatus(MprisStatus::Playing)]
        ));
        assert_eq!(seeked, None);

        let (props, seeked) = inner.apply(
            Update::Status(PlaybackStatus::Playing { position_ms: 1_200 }),
            now + Duration::from_millis(1_000),
        );
        assert!(props.is_empty());
        assert_eq!(seeked, None);

        let (_, seeked) = inner.apply(
            Update::Status(PlaybackStatus::Playing {
                position_ms: 60_000,
            }),
            now + Duration::from_millis(2_000),
        );
        assert_eq!(seeked, Some(Time::from_millis(60_000)));
    }

    #[test]
    fn volume_update_only_emits_on_change() {
        let (inner, _channel) = inner(Box::new(|_| {}));
        let now = Instant::now();
        let (props, _) = inner.apply(Update::Volume(1.0), now);
        assert!(props.is_empty());
        let (props, _) = inner.apply(Update::Volume(0.5), now);
        assert!(matches!(props.as_slice(), [Property::Volume(v)] if *v == 0.5));
    }

    #[test]
    fn transport_requires_a_track() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&events);
        let (inner, (_tx, mut rx)) = inner(Box::new(move |event| {
            sink.lock().unwrap().push(event);
        }));

        inner.transport(MediaControlEvent::Toggle, |_| MprisStatus::Playing);
        assert!(events.lock().unwrap().is_empty());
        assert_eq!(inner.state().status, MprisStatus::Stopped);

        inner.apply(Update::Metadata(song()), Instant::now());
        inner.transport(MediaControlEvent::Toggle, |_| MprisStatus::Playing);
        assert_eq!(*events.lock().unwrap(), vec![MediaControlEvent::Toggle]);
        assert_eq!(inner.state().status, MprisStatus::Playing);
        assert!(matches!(
            rx.try_recv(),
            Ok(Update::Emit(props)) if matches!(props.as_slice(), [Property::PlaybackStatus(MprisStatus::Playing)])
        ));
    }
}
