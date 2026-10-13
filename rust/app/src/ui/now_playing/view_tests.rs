use std::sync::Arc;
use std::time::{Duration, Instant};

use feishin_core::model::Song;
use i_slint_backend_testing::{TestingBackend, TestingBackendOptions};
use slint::{ComponentHandle, Model, SharedString};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::connect::DemoConnector;
use crate::controller::{Command, QueueEntry};
use crate::covers::DemoCovers;
use crate::demo::FakeServer;
use crate::session::Session;
use crate::store::JsonStore;
use crate::ui::{
    AppState, AppWindow, Context, LyricsStatus, NowPlayingState, Page, PlayerState, Route, queue,
};

struct NowPlayingPage {
    window: AppWindow,
    commands: UnboundedReceiver<Command>,
    _runtime: tokio::runtime::Runtime,
    _dir: tempfile::TempDir,
}

fn claim_testing_backend() -> bool {
    let backend = TestingBackend::new(TestingBackendOptions {
        threading: true,
        ..TestingBackendOptions::default()
    });
    slint::platform::set_platform(Box::new(backend)).is_ok()
}

fn open_now_playing_page() -> NowPlayingPage {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let session = Session::default();
    session.set(Some(Arc::new(FakeServer::new(100))));
    let (sender, commands) = tokio::sync::mpsc::unbounded_channel();
    let ctx = Context {
        rt: runtime.handle().clone(),
        commands: sender,
        session,
        store: Arc::new(JsonStore::new(dir.path())),
        connector: Arc::new(DemoConnector::default()),
        covers: Arc::new(DemoCovers),
    };
    let window = AppWindow::new().unwrap();
    crate::ui::install(&window, &ctx, &Default::default());
    let player = window.global::<PlayerState>();
    player.set_has_song(true);
    player.set_song_id("song-3".into());
    player.set_title("Song three".into());
    window.global::<AppState>().set_page(Page::Main);
    window
        .global::<AppState>()
        .invoke_navigate(Route::NowPlaying, SharedString::default());
    window.show().unwrap();
    NowPlayingPage {
        window,
        commands,
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

fn wait_until_loaded(window: &AppWindow) {
    let deadline = Instant::now() + Duration::from_secs(10);
    let weak_window = window.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(20),
        move || {
            let Some(window) = weak_window.upgrade() else {
                return;
            };
            if is_fully_loaded(&window.global::<NowPlayingState>()) || Instant::now() > deadline {
                let _ = slint::quit_event_loop();
            }
        },
    );
    slint::run_event_loop().unwrap();
    timer.stop();
}

fn settle() {
    slint::Timer::single_shot(Duration::from_millis(50), || {
        let _ = slint::quit_event_loop();
    });
    slint::run_event_loop().unwrap();
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

fn queue_entry(uid: u64) -> QueueEntry {
    QueueEntry {
        uid,
        song_id: format!("song-{uid}"),
        title: String::new(),
        artists: String::new(),
        duration_ms: 0,
    }
}

fn assert_up_next_follows_queue(window: &AppWindow, state: &NowPlayingState) {
    queue::set_entries(window, (1..=5).map(queue_entry).collect(), Some(2));
    settle();
    assert_eq!(state.get_up_next().row_count(), 4);
    queue::set_current(window, Some(4));
    settle();
    assert_eq!(state.get_up_next().row_count(), 2);
    assert_eq!(
        state.get_up_next().row_data(0).map(|row| row.uid),
        Some("4".into())
    );
}

fn played_songs(commands: &mut UnboundedReceiver<Command>) -> Option<(Vec<Song>, Option<String>)> {
    std::iter::from_fn(|| commands.try_recv().ok()).find_map(|command| match command {
        Command::PlaySongs { songs, song_id } => Some((songs, song_id)),
        _ => None,
    })
}

#[test]
fn view_shows_demo_lyrics_and_similar_songs() {
    if !claim_testing_backend() {
        eprintln!("skipped: another test owns the Slint event loop");
        return;
    }
    let mut page = open_now_playing_page();
    let window = &page.window;
    wait_until_loaded(window);
    let state = window.global::<NowPlayingState>();
    assert_song_details(&state);

    window.global::<PlayerState>().set_position_ms(6_500);
    settle();
    assert_eq!(state.get_active_line(), 1);

    assert_up_next_follows_queue(window, &state);

    let similar = state.get_similar().row_data(0).unwrap();
    state.invoke_play_similar(similar.id.clone());
    let Some((songs, song_id)) = played_songs(&mut page.commands) else {
        panic!("expected PlaySongs");
    };
    assert_eq!(songs.len(), state.get_similar().row_count());
    assert_eq!(song_id.as_deref(), Some(similar.id.as_str()));
}
