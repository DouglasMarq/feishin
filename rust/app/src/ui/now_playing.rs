use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::LazyLock;

use feishin_api::MusicServer;
use feishin_core::model::Song;
use feishin_lyrics::{LyricLine, LyricsQuery, LyricsSource};
use slint::{ComponentHandle, Model, ModelNotify, ModelRc, ModelTracker, SharedString, VecModel};

use super::{
    AppWindow, ArtistLink, Context, LyricRow, LyricsStatus, NowPlayingState, PlayerState, QueueRow,
    QueueState, TrackRow, clamp_i32, on_ui, player, tracks,
};
use crate::controller::{Command, artist_names};

const COVER_SIZE: u32 = 600;
const SIMILAR_COUNT: usize = 20;
const LYRICS_CACHE_SIZE: usize = 32;

static HTTP: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);

/// One provider in the configured lookup order; adjacent remote providers are grouped into one fetch.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    Server,
    Remote(Vec<LyricsSource>),
}

fn lyrics_plan(names: &[String]) -> Vec<Step> {
    let mut plan: Vec<Step> = Vec::new();
    for name in names {
        let source = match name.trim().to_ascii_lowercase().as_str() {
            "server" => {
                if !plan.contains(&Step::Server) {
                    plan.push(Step::Server);
                }
                continue;
            }
            "lrclib" => LyricsSource::Lrclib,
            "netease" => LyricsSource::NetEase,
            "genius" => LyricsSource::Genius,
            other => {
                tracing::debug!("unknown lyrics source {other}");
                continue;
            }
        };
        let seen = plan
            .iter()
            .any(|step| matches!(step, Step::Remote(sources) if sources.contains(&source)));
        if seen {
            continue;
        }
        match plan.last_mut() {
            Some(Step::Remote(sources)) => sources.push(source),
            _ => plan.push(Step::Remote(vec![source])),
        }
    }
    plan
}

fn source_label(source: LyricsSource) -> &'static str {
    match source {
        LyricsSource::Lrclib => "LRCLIB",
        LyricsSource::NetEase => "NetEase",
        LyricsSource::Genius => "Genius",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SongLyrics {
    source: &'static str,
    synced: bool,
    lines: Vec<LyricLine>,
}

impl SongLyrics {
    fn from_text(source: &'static str, text: &str) -> Option<Self> {
        let lines = feishin_lyrics::parse_lrc(text);
        if lines.iter().all(|line| line.text.trim().is_empty()) {
            return None;
        }
        let synced = lines.iter().any(|line| line.time_ms.is_some());
        Some(Self {
            source,
            synced,
            lines,
        })
    }

    fn times(&self) -> Vec<u64> {
        if !self.synced {
            return Vec::new();
        }
        self.lines
            .iter()
            .map(|line| line.time_ms.unwrap_or(0))
            .collect()
    }
}

fn lyrics_query(song: &Song) -> LyricsQuery {
    LyricsQuery {
        title: song.title.clone(),
        artist: artist_names(song),
        album: song.album.clone(),
        duration_ms: Some(song.duration_ms).filter(|ms| *ms > 0),
    }
}

/// Walks `plan` and returns the first lyrics found; errors only when nothing was found and a step failed.
async fn resolve_lyrics(
    server: &dyn MusicServer,
    song: &Song,
    plan: &[Step],
) -> Result<Option<SongLyrics>, String> {
    let mut error = None;
    for step in plan {
        match step {
            Step::Server => match server.lyrics(song).await {
                Ok(Some(text)) => {
                    if let Some(lyrics) = SongLyrics::from_text("Server", &text) {
                        return Ok(Some(lyrics));
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::debug!("server lyrics for {}: {e}", song.id);
                    error.get_or_insert(e.to_string());
                }
            },
            Step::Remote(sources) => {
                match feishin_lyrics::fetch(&HTTP, &lyrics_query(song), sources).await {
                    Ok(Some(found)) => {
                        return Ok(Some(SongLyrics {
                            source: source_label(found.source),
                            synced: found.synced,
                            lines: found.lines,
                        }));
                    }
                    Ok(None) => {}
                    Err(e) => {
                        error.get_or_insert(e.to_string());
                    }
                }
            }
        }
    }
    error.map_or(Ok(None), Err)
}

/// Index of the line playing at `position_ms`, or `None` before the first line.
fn active_line(times: &[u64], position_ms: u64) -> Option<usize> {
    times.partition_point(|&t| t <= position_ms).checked_sub(1)
}

/// Lyrics lookups by song id, most recent last. `None` records that nothing was found.
struct LyricsCache {
    entries: VecDeque<(String, Option<Rc<SongLyrics>>)>,
    capacity: usize,
}

impl LyricsCache {
    fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    fn get(&mut self, song_id: &str) -> Option<Option<Rc<SongLyrics>>> {
        let index = self.entries.iter().position(|(id, _)| id == song_id)?;
        let entry = self.entries.remove(index)?;
        let lyrics = entry.1.clone();
        self.entries.push_back(entry);
        Some(lyrics)
    }

    fn insert(&mut self, song_id: String, lyrics: Option<Rc<SongLyrics>>) {
        self.entries.retain(|(id, _)| *id != song_id);
        if self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back((song_id, lyrics));
    }
}

/// The queue from the current item onward, read lazily from the queue panel's rows.
pub struct UpNextModel {
    queue: ModelRc<QueueRow>,
    offset: Cell<Option<usize>>,
    notify: ModelNotify,
}

impl UpNextModel {
    fn new(queue: ModelRc<QueueRow>) -> Self {
        Self {
            queue,
            offset: Cell::new(None),
            notify: ModelNotify::default(),
        }
    }

    fn is_current(&self, index: usize) -> bool {
        self.queue.row_data(index).is_some_and(|row| row.current)
    }

    fn locate(&self) -> Option<usize> {
        if let Some(offset) = self.offset.get()
            && self.is_current(offset)
        {
            return Some(offset);
        }
        (0..self.queue.row_count()).find(|&index| self.is_current(index))
    }

    pub fn refresh(&self) -> Option<usize> {
        let offset = self.locate();
        self.offset.set(offset);
        self.notify.reset();
        offset
    }
}

impl Model for UpNextModel {
    type Data = QueueRow;

    fn row_count(&self) -> usize {
        self.offset
            .get()
            .map_or(0, |offset| self.queue.row_count().saturating_sub(offset))
    }

    fn row_data(&self, row: usize) -> Option<QueueRow> {
        self.queue.row_data(self.offset.get()? + row)
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Default)]
struct View {
    cache: RefCell<Option<LyricsCache>>,
    times: RefCell<Vec<u64>>,
    similar: RefCell<Vec<Song>>,
}

impl View {
    fn cached(&self, song_id: &str) -> Option<Option<Rc<SongLyrics>>> {
        self.cache.borrow_mut().as_mut()?.get(song_id)
    }

    fn remember(&self, song_id: String, lyrics: Option<Rc<SongLyrics>>) {
        self.cache
            .borrow_mut()
            .get_or_insert_with(|| LyricsCache::new(LYRICS_CACHE_SIZE))
            .insert(song_id, lyrics);
    }

    fn set_active(&self, window: &AppWindow) {
        let position_ms = u64::try_from(window.global::<PlayerState>().get_position_ms());
        let line = active_line(&self.times.borrow(), position_ms.unwrap_or(0));
        let line = line.map_or(-1, |line| clamp_i32(line as u64));
        let state = window.global::<NowPlayingState>();
        if state.get_active_line() != line {
            state.set_active_line(line);
        }
    }

    fn show_lyrics(&self, window: &AppWindow, lyrics: Option<&SongLyrics>) {
        let state = window.global::<NowPlayingState>();
        let Some(lyrics) = lyrics else {
            *self.times.borrow_mut() = Vec::new();
            state.set_lyrics(ModelRc::default());
            state.set_lyrics_status(LyricsStatus::Missing);
            return;
        };
        let rows: Vec<LyricRow> = lyrics
            .lines
            .iter()
            .map(|line| LyricRow {
                text: line.text.as_str().into(),
                time_ms: line.time_ms.map_or(-1, clamp_i32),
            })
            .collect();
        *self.times.borrow_mut() = lyrics.times();
        state.set_lyrics(ModelRc::new(VecModel::from(rows)));
        state.set_lyrics_synced(lyrics.synced);
        state.set_lyrics_source(lyrics.source.into());
        state.set_lyrics_status(LyricsStatus::Found);
        state.set_active_line(-1);
        self.set_active(window);
    }
}

fn refresh_up_next(window: &AppWindow) {
    let state = window.global::<NowPlayingState>();
    let rows = state.get_up_next();
    if let Some(model) = rows.as_any().downcast_ref::<UpNextModel>() {
        let offset = model.refresh();
        state.set_up_next_offset(offset.map_or(-1, |offset| clamp_i32(offset as u64)));
    }
}

fn clear(window: &AppWindow, view: &View, song_id: &str) {
    let state = window.global::<NowPlayingState>();
    state.set_song_id(song_id.into());
    state.set_loaded(false);
    state.set_artists(ModelRc::default());
    state.set_album_id(SharedString::default());
    state.set_has_cover(false);
    state.set_cover(slint::Image::default());
    state.set_lyrics(ModelRc::default());
    state.set_lyrics_synced(false);
    state.set_lyrics_source(SharedString::default());
    state.set_lyrics_status(LyricsStatus::Idle);
    state.set_active_line(-1);
    state.set_similar(ModelRc::default());
    state.set_similar_loading(false);
    view.times.borrow_mut().clear();
    view.similar.borrow_mut().clear();
}

fn is_current(window: &AppWindow, song_id: &str) -> bool {
    window.global::<NowPlayingState>().get_song_id() == song_id
}

fn load(window: &AppWindow, ctx: &Context, view: &Rc<View>, song_id: &str) {
    refresh_up_next(window);
    let state = window.global::<NowPlayingState>();
    if state.get_song_id() == song_id && (state.get_loaded() || song_id.is_empty()) {
        return;
    }
    clear(window, view, song_id);
    let Some(server) = ctx.session.get().filter(|_| !song_id.is_empty()) else {
        return;
    };

    let cached = view.cached(song_id);
    match &cached {
        Some(lyrics) => view.show_lyrics(window, lyrics.as_deref()),
        None => state.set_lyrics_status(LyricsStatus::Loading),
    }
    state.set_similar_loading(true);

    let weak = window.as_weak();
    let ctx = ctx.clone();
    let song_id = song_id.to_owned();
    let want_lyrics = cached.is_none();
    let rt = ctx.rt.clone();
    rt.spawn(async move {
        let song = match server.song(&song_id).await {
            Ok(song) => song,
            Err(e) => {
                tracing::warn!("now playing {song_id}: {e}");
                on_ui(&weak, move |window| {
                    if is_current(window, &song_id) {
                        let state = window.global::<NowPlayingState>();
                        state.set_similar_loading(false);
                        if state.get_lyrics_status() == LyricsStatus::Loading {
                            state.set_lyrics_status(LyricsStatus::Failed);
                        }
                    }
                });
                return;
            }
        };
        show_song(&weak, &song);
        tokio::join!(
            fetch_cover(&weak, &ctx, server.as_ref(), &song),
            fetch_similar(&weak, server.as_ref(), &song_id),
            async {
                if want_lyrics {
                    fetch_lyrics(&weak, &ctx, server.as_ref(), &song).await;
                }
            },
        );
    });
}

fn show_song(weak: &slint::Weak<AppWindow>, song: &Song) {
    let song_id = song.id.clone();
    let artists: Vec<ArtistLink> = song
        .artists
        .iter()
        .map(|artist| ArtistLink {
            id: artist.id.as_str().into(),
            name: artist.name.as_str().into(),
        })
        .collect();
    let album_id = song.album_id.clone().unwrap_or_default();
    on_ui(weak, move |window| {
        if !is_current(window, &song_id) {
            return;
        }
        let state = window.global::<NowPlayingState>();
        state.set_artists(ModelRc::new(VecModel::from(artists)));
        state.set_album_id(album_id.into());
        state.set_loaded(true);
    });
}

async fn fetch_cover(
    weak: &slint::Weak<AppWindow>,
    ctx: &Context,
    server: &dyn MusicServer,
    song: &Song,
) {
    let Some(cover_id) = song.cover_art_id.as_deref() else {
        return;
    };
    match ctx.covers.cover(server, cover_id, COVER_SIZE).await {
        Ok(cover) => {
            let song_id = song.id.clone();
            on_ui(weak, move |window| {
                if is_current(window, &song_id)
                    && let Some(image) = player::to_image(&cover)
                {
                    let state = window.global::<NowPlayingState>();
                    state.set_cover(image);
                    state.set_has_cover(true);
                }
            });
        }
        Err(e) => tracing::debug!("cover {cover_id}: {e:#}"),
    }
}

async fn fetch_similar(weak: &slint::Weak<AppWindow>, server: &dyn MusicServer, song_id: &str) {
    let songs = server
        .similar_songs(song_id, SIMILAR_COUNT)
        .await
        .unwrap_or_else(|e| {
            tracing::debug!("similar songs for {song_id}: {e}");
            Vec::new()
        });
    let song_id = song_id.to_owned();
    on_ui(weak, move |window| {
        if !is_current(window, &song_id) {
            return;
        }
        let rows: Vec<TrackRow> = songs
            .iter()
            .enumerate()
            .map(|(index, song)| tracks::track_row(song, index))
            .collect();
        let state = window.global::<NowPlayingState>();
        state.set_similar(ModelRc::new(VecModel::from(rows)));
        state.set_similar_loading(false);
        with_view(|view| *view.similar.borrow_mut() = songs);
    });
}

async fn fetch_lyrics(
    weak: &slint::Weak<AppWindow>,
    ctx: &Context,
    server: &dyn MusicServer,
    song: &Song,
) {
    let names = match ctx.store.load_settings() {
        Ok(settings) => settings.integrations.lyrics_sources,
        Err(e) => {
            tracing::debug!("settings: {e:#}");
            feishin_storage::settings::IntegrationSettings::default().lyrics_sources
        }
    };
    let result = resolve_lyrics(server, song, &lyrics_plan(&names)).await;
    let song_id = song.id.clone();
    on_ui(weak, move |window| {
        with_view(|view| {
            let lyrics = match result {
                Ok(lyrics) => {
                    let lyrics = lyrics.map(Rc::new);
                    view.remember(song_id.clone(), lyrics.clone());
                    lyrics
                }
                Err(e) => {
                    tracing::warn!("lyrics for {song_id}: {e}");
                    if is_current(window, &song_id) {
                        window
                            .global::<NowPlayingState>()
                            .set_lyrics_status(LyricsStatus::Failed);
                    }
                    return;
                }
            };
            if is_current(window, &song_id) {
                view.show_lyrics(window, lyrics.as_deref());
            }
        });
    });
}

thread_local! {
    static VIEW: RefCell<Option<Rc<View>>> = const { RefCell::new(None) };
}

fn with_view(f: impl FnOnce(&View)) {
    if let Some(view) = VIEW.with(|view| view.borrow().clone()) {
        f(&view);
    }
}

pub fn install(window: &AppWindow, ctx: &Context) {
    let view = Rc::new(View::default());
    VIEW.with(|slot| *slot.borrow_mut() = Some(Rc::clone(&view)));
    let state = window.global::<NowPlayingState>();
    let queue = window.global::<QueueState>().get_rows();
    state.set_up_next(ModelRc::new(UpNextModel::new(queue)));

    let weak = window.as_weak();
    let load_ctx = ctx.clone();
    let load_view = Rc::clone(&view);
    state.on_load(move |song_id| {
        if let Some(window) = weak.upgrade() {
            load(&window, &load_ctx, &load_view, &song_id);
        }
    });

    let weak = window.as_weak();
    let position_view = Rc::clone(&view);
    state.on_position_changed(move || {
        if let Some(window) = weak.upgrade() {
            position_view.set_active(&window);
        }
    });

    let weak = window.as_weak();
    state.on_refresh_up_next(move || {
        if let Some(window) = weak.upgrade() {
            refresh_up_next(&window);
        }
    });

    let play_ctx = ctx.clone();
    state.on_play_similar(move |song_id| {
        let songs = view.similar.borrow().clone();
        if !songs.is_empty() {
            play_ctx.send(Command::PlaySongs {
                songs,
                song_id: Some(song_id.to_string()),
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::demo::FakeServer;

    fn names(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn plan_follows_settings_order_and_groups_remote_sources() {
        assert_eq!(
            lyrics_plan(&names(&["server", "lrclib", "NetEase", "bogus", "lrclib"])),
            vec![
                Step::Server,
                Step::Remote(vec![LyricsSource::Lrclib, LyricsSource::NetEase])
            ]
        );
        assert_eq!(
            lyrics_plan(&names(&["genius", "server", "lrclib"])),
            vec![
                Step::Remote(vec![LyricsSource::Genius]),
                Step::Server,
                Step::Remote(vec![LyricsSource::Lrclib]),
            ]
        );
        assert!(lyrics_plan(&[]).is_empty());
    }

    #[test]
    fn active_line_tracks_the_last_started_line() {
        let times = [1_000, 5_000, 5_000, 9_000];
        assert_eq!(active_line(&times, 0), None);
        assert_eq!(active_line(&times, 1_000), Some(0));
        assert_eq!(active_line(&times, 6_000), Some(2));
        assert_eq!(active_line(&times, 60_000), Some(3));
        assert_eq!(active_line(&[], 1_000), None);
    }

    #[test]
    fn lyrics_text_maps_to_synced_or_plain() {
        let synced = SongLyrics::from_text("Server", "[00:01.00]One\n[00:02.50]Two").unwrap();
        assert!(synced.synced);
        assert_eq!(synced.times(), vec![1_000, 2_500]);
        let plain = SongLyrics::from_text("Server", "One\nTwo").unwrap();
        assert!(!plain.synced);
        assert!(plain.times().is_empty());
        assert_eq!(SongLyrics::from_text("Server", "  \n"), None);
    }

    #[test]
    fn cache_is_bounded_and_keeps_recent_entries() {
        let mut cache = LyricsCache::new(2);
        cache.insert("a".into(), None);
        cache.insert("b".into(), None);
        assert_eq!(cache.get("a"), Some(None));
        cache.insert("c".into(), None);
        assert!(cache.get("b").is_none());
        assert!(cache.get("a").is_some());
        assert!(cache.get("c").is_some());
    }

    #[tokio::test]
    async fn server_lyrics_come_first() {
        let server = FakeServer::new(100);
        let song = server.song("song-3").await.unwrap();
        let lyrics = resolve_lyrics(&server, &song, &lyrics_plan(&names(&["server"])))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(lyrics.source, "Server");
        assert!(lyrics.synced);
        assert!(resolve_lyrics(&server, &song, &[]).await.unwrap().is_none());
    }

    #[test]
    fn up_next_starts_at_the_current_row() {
        let row = |uid: &str, current: bool| QueueRow {
            uid: uid.into(),
            current,
            ..QueueRow::default()
        };
        let queue = Rc::new(VecModel::from(vec![
            row("1", false),
            row("2", true),
            row("3", false),
        ]));
        let model = UpNextModel::new(ModelRc::from(Rc::clone(&queue)));
        assert_eq!(model.row_count(), 0);
        model.refresh();
        assert_eq!(model.row_count(), 2);
        assert_eq!(model.row_data(0).map(|r| r.uid), Some("2".into()));
        queue.set_row_data(1, row("2", false));
        queue.set_row_data(2, row("3", true));
        model.refresh();
        assert_eq!(model.row_count(), 1);
        assert_eq!(model.row_data(0).map(|r| r.uid), Some("3".into()));
    }

    /// Renders the now-playing route against the fake server on the testing backend.
    #[test]
    fn view_shows_demo_lyrics_and_similar_songs() {
        use std::time::{Duration, Instant};

        use i_slint_backend_testing::{TestingBackend, TestingBackendOptions};

        use crate::connect::DemoConnector;
        use crate::covers::DemoCovers;
        use crate::session::Session;
        use crate::store::JsonStore;
        use crate::ui::{AppState, Page, Route};

        let backend = TestingBackend::new(TestingBackendOptions {
            threading: true,
            ..TestingBackendOptions::default()
        });
        if slint::platform::set_platform(Box::new(backend)).is_err() {
            eprintln!("skipped: another test owns the Slint event loop");
            return;
        }
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let session = Session::default();
        session.set(Some(Arc::new(FakeServer::new(100))));
        let (sender, mut commands) = tokio::sync::mpsc::unbounded_channel();
        let ctx = Context {
            rt: rt.handle().clone(),
            commands: sender,
            session,
            store: Arc::new(JsonStore::new(dir.path())),
            connector: Arc::new(DemoConnector),
            covers: Arc::new(DemoCovers),
        };
        let window = AppWindow::new().unwrap();
        super::super::install(&window, &ctx, &Default::default());
        let player = window.global::<PlayerState>();
        player.set_has_song(true);
        player.set_song_id("song-3".into());
        player.set_title("Song three".into());
        window.global::<AppState>().set_page(Page::Main);
        window
            .global::<AppState>()
            .invoke_navigate(Route::NowPlaying, SharedString::default());
        window.show().unwrap();

        let deadline = Instant::now() + Duration::from_secs(10);
        let weak = window.as_weak();
        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(20),
            move || {
                let Some(window) = weak.upgrade() else {
                    return;
                };
                let state = window.global::<NowPlayingState>();
                let ready = state.get_lyrics_status() == LyricsStatus::Found
                    && state.get_similar().row_count() > 0
                    && state.get_loaded()
                    && state.get_has_cover();
                if ready || Instant::now() > deadline {
                    let _ = slint::quit_event_loop();
                }
            },
        );
        slint::run_event_loop().unwrap();
        timer.stop();

        let state = window.global::<NowPlayingState>();
        assert_eq!(state.get_song_id(), "song-3");
        assert_eq!(state.get_lyrics_status(), LyricsStatus::Found);
        assert_eq!(state.get_lyrics_source(), "Server");
        assert!(state.get_lyrics_synced());
        assert!(state.get_lyrics().row_count() > 3);
        assert_eq!(state.get_album_id(), "album-0");
        assert_eq!(state.get_artists().row_count(), 1);
        assert!(state.get_has_cover());

        player.set_position_ms(6_500);
        let settle = || {
            slint::Timer::single_shot(Duration::from_millis(50), || {
                let _ = slint::quit_event_loop();
            });
            slint::run_event_loop().unwrap();
        };
        settle();
        assert_eq!(state.get_active_line(), 1);

        let entry = |uid| crate::controller::QueueEntry {
            uid,
            song_id: format!("song-{uid}"),
            title: String::new(),
            artists: String::new(),
            duration_ms: 0,
        };
        super::super::queue::set_entries(&window, (1..=5).map(entry).collect(), Some(2));
        settle();
        assert_eq!(state.get_up_next().row_count(), 4);
        super::super::queue::set_current(&window, Some(4));
        settle();
        assert_eq!(state.get_up_next().row_count(), 2);
        assert_eq!(
            state.get_up_next().row_data(0).map(|row| row.uid),
            Some("4".into())
        );

        let similar = state.get_similar().row_data(0).unwrap();
        state.invoke_play_similar(similar.id.clone());
        let played =
            std::iter::from_fn(|| commands.try_recv().ok()).find_map(|command| match command {
                Command::PlaySongs { songs, song_id } => Some((songs, song_id)),
                _ => None,
            });
        let Some((songs, song_id)) = played else {
            panic!("expected PlaySongs");
        };
        assert_eq!(songs.len(), state.get_similar().row_count());
        assert_eq!(song_id.as_deref(), Some(similar.id.as_str()));
    }
}
