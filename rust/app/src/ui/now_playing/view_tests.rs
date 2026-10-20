use std::sync::Arc;
use std::time::Duration;

use feishin_core::model::Song;
use feishin_core::queue::{Queue, QueueTrack, YouTubeTrack};
use feishin_storage::settings::{LyricsSourceSetting, Settings};
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::{ComponentHandle, Model, SharedString};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::connect::DemoConnector;
use crate::controller::Command;
use crate::covers::DemoCovers;
use crate::demo::{FakeServer, test_song};
use crate::session::Session;
use crate::store::JsonStore;
use crate::ui::test_window::{Jobs, SIZE, claim_routed_platform, pump_until, routed_runtime};
use crate::ui::{
    AppState, AppWindow, Context, LyricsStatus, NowPlayingState, Page, PlayerState, Route, queue,
};

struct NowPlayingPage {
    window: AppWindow,
    commands: UnboundedReceiver<Command>,
    screen: Screen,
    _runtime: tokio::runtime::Runtime,
    _dir: tempfile::TempDir,
}

struct Screen {
    software_window: std::rc::Rc<MinimalSoftwareWindow>,
    jobs: Jobs,
}

impl Screen {
    fn claim() -> Option<Self> {
        let software_window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        let jobs = claim_routed_platform(&software_window)?;
        Some(Self {
            software_window,
            jobs,
        })
    }

    fn wait_until_loaded(&self, window: &AppWindow) {
        self.wait_until(window, is_fully_loaded);
    }

    fn wait_until(&self, window: &AppWindow, is_ready: fn(&NowPlayingState) -> bool) {
        let state = window.global::<NowPlayingState>();
        pump_until(
            &self.software_window,
            &self.jobs,
            Duration::from_secs(10),
            || is_ready(&state),
        );
    }

    fn settle(&self) {
        pump_until(
            &self.software_window,
            &self.jobs,
            Duration::from_millis(50),
            || false,
        );
    }
}

fn open_now_playing_page(screen: Screen) -> NowPlayingPage {
    let runtime = routed_runtime(1);
    let dir = tempfile::tempdir().unwrap();
    let session = Session::default();
    session.set(Some(Arc::new(FakeServer::new(100))));
    let (sender, commands) = tokio::sync::mpsc::unbounded_channel();
    let store = JsonStore::new(dir.path());
    let mut settings = Settings::default();
    settings.integrations.lyrics_sources = vec![LyricsSourceSetting::Server];
    session.publish_settings(&settings);
    let context = Context {
        runtime: runtime.handle().clone(),
        commands: sender,
        session,
        store: Arc::new(store),
        connector: Arc::new(DemoConnector::default()),
        covers: Arc::new(DemoCovers),
        remote: crate::remote::Remote::default(),
    };
    let window = AppWindow::new().unwrap();
    crate::ui::install(&window, &context, &Default::default());
    let player = window.global::<PlayerState>();
    player.set_has_song(true);
    player.set_song_id("song-3".into());
    player.set_title("Song three".into());
    window.global::<AppState>().set_page(Page::Main);
    window
        .global::<AppState>()
        .invoke_navigate(Route::NowPlaying, SharedString::default());
    window.show().unwrap();
    screen.software_window.set_size(SIZE);
    NowPlayingPage {
        window,
        commands,
        screen,
        _runtime: runtime,
        _dir: dir,
    }
}

fn is_fully_loaded(state: &NowPlayingState) -> bool {
    state.get_lyrics_status() == LyricsStatus::Found
        && state.get_similar().row_count() > 0
        && state.get_loaded()
        && state.get_has_cover()
}

fn assert_song_details(state: &NowPlayingState) {
    assert_eq!(state.get_song_id(), "song-3");
    assert_eq!(state.get_lyrics_status(), LyricsStatus::Found);
    assert_eq!(state.get_lyrics_source(), "Server");
    assert!(state.get_lyrics_synced());
    assert!(state.get_lyrics().row_count() > 3);
    assert_eq!(state.get_album_id(), "album-0");
    assert_eq!(state.get_artists().row_count(), 1);
    assert!(state.get_has_cover());
}

fn queue_of_five(current: &str) -> Queue {
    let mut queue = Queue::with_seed(1);
    let songs = (1..=5)
        .map(|index| test_song(&format!("song-{index}"), 1_000))
        .collect();
    queue.play_now(songs, Some(current));
    queue
}

fn assert_up_next_follows_queue(page: &NowPlayingPage, state: &NowPlayingState) {
    let window = &page.window;
    let mut queue = queue_of_five("song-2");
    queue::set_view(window, queue.view());
    page.screen.settle();
    assert_eq!(state.get_up_next().row_count(), 4);
    queue.play_uid(4);
    queue::set_view(window, queue.view());
    page.screen.settle();
    assert_eq!(state.get_up_next().row_count(), 2);
    assert_eq!(
        state.get_up_next().row_data(0).map(|row| row.uid),
        Some("4".into())
    );
    queue.remove(&[5]);
    queue::set_view(window, queue.view());
    page.screen.settle();
    assert_eq!(state.get_up_next().row_count(), 1);
}

fn played_songs(commands: &mut UnboundedReceiver<Command>) -> Option<(Vec<Song>, Option<String>)> {
    std::iter::from_fn(|| commands.try_recv().ok()).find_map(|command| match command {
        Command::PlaySongs { songs, song_id } => Some((songs, song_id)),
        _ => None,
    })
}

fn is_youtube_track_loaded(state: &NowPlayingState) -> bool {
    state.get_external()
        && state.get_loaded()
        && state.get_has_cover()
        && state.get_lyrics_status() == LyricsStatus::Missing
}

fn assert_youtube_track_details(page: &NowPlayingPage, state: &NowPlayingState) {
    let window = &page.window;
    let mut queue = Queue::with_seed(2);
    let track = QueueTrack::from(&YouTubeTrack {
        video_id: "abc".into(),
        title: "Video".into(),
        artists: vec!["Singer".into()],
        thumbnail_url: Some("https://i.ytimg.com/vi/abc/hqdefault.jpg".into()),
        ..YouTubeTrack::default()
    });
    queue.play_tracks_now(vec![track], Some(0));
    queue::set_view(window, queue.view());
    window.global::<PlayerState>().set_song_id("yt:abc".into());
    page.screen.wait_until(window, is_youtube_track_loaded);
    assert!(is_youtube_track_loaded(state));
    let artists: Vec<_> = state.get_artists().iter().collect();
    assert_eq!(artists.len(), 1);
    assert_eq!(artists[0].name, "Singer");
    assert_eq!(artists[0].id, "");
    assert_eq!(state.get_album_id(), "");
    assert_eq!(state.get_similar().row_count(), 0);
    assert!(!state.get_similar_loading());
}

#[test]
fn view_shows_demo_lyrics_and_similar_songs() {
    let Some(screen) = Screen::claim() else {
        eprintln!("skipped: another test owns the Slint event loop");
        return;
    };
    let mut page = open_now_playing_page(screen);
    let window = &page.window;
    page.screen.wait_until_loaded(window);
    let state = window.global::<NowPlayingState>();
    assert_song_details(&state);

    window.global::<PlayerState>().set_position_ms(6_500);
    page.screen.settle();
    assert_eq!(state.get_active_line(), 1);

    assert_up_next_follows_queue(&page, &state);

    let similar = state.get_similar().row_data(0).unwrap();
    state.invoke_play_similar(similar.id.clone());
    let Some((songs, song_id)) = played_songs(&mut page.commands) else {
        panic!("expected PlaySongs");
    };
    assert_eq!(songs.len(), state.get_similar().row_count());
    assert_eq!(song_id.as_deref(), Some(similar.id.as_str()));

    assert_youtube_track_details(&page, &state);
}
