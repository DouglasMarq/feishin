use std::time::Duration;

use feishin_core::model::SongListQuery;
use feishin_storage::settings::UiSettings;
use tokio::sync::oneshot;

use super::harness::{AudioCall, Harness, start_harness_with, test_settings};
use crate::controller::{Command, PersistenceEvent};
use crate::list_stream::{ListRow, SongSource};
use crate::store::Store;

const SONGS: usize = 1234;
const FIRST_PAGE: usize = 500;

async fn streaming_harness() -> Harness {
    let mut harness = start_harness_with(test_settings(false, false), SONGS, None);
    harness.controller.handle(Command::PlayFromList {
        source: SongSource::Library(SongListQuery::default()),
        song_id: None,
        row_index: ListRow(700),
    });
    harness.handle_next_command().await;
    assert_eq!(harness.controller.queue().len(), FIRST_PAGE);
    harness
}

fn request_shutdown(harness: &mut Harness) -> oneshot::Receiver<()> {
    let (done, finished) = oneshot::channel();
    let ui = UiSettings::default();
    assert!(harness.controller.handle(Command::Shutdown { ui, done }));
    finished
}

async fn run_until_stopped(harness: &mut Harness) {
    loop {
        let next = tokio::time::timeout(Duration::from_secs(10), harness.commands.recv()).await;
        let Ok(Some(command)) = next else {
            panic!("the controller never finished shutting down");
        };
        if !harness.controller.handle(command) {
            return;
        }
    }
}

fn saved_song_count(harness: &Harness) -> Option<usize> {
    let stored = harness.store.load_queue().ok().flatten()?;
    Some(stored.snapshot.items.len())
}

#[tokio::test]
async fn quitting_while_a_list_streams_in_saves_the_whole_list() {
    let mut harness = streaming_harness().await;
    let finished = request_shutdown(&mut harness);
    run_until_stopped(&mut harness).await;
    assert!(finished.await.is_ok());
    assert_eq!(saved_song_count(&harness), Some(SONGS));
}

#[tokio::test]
async fn a_stream_still_loading_at_the_deadline_is_saved_as_far_as_it_got() {
    let mut harness = streaming_harness().await;
    let finished = request_shutdown(&mut harness);
    let deadline = Command::Persistence(PersistenceEvent::StreamDrainDeadline);
    assert!(!harness.controller.handle(deadline));
    assert!(finished.await.is_ok());
    assert_eq!(saved_song_count(&harness), Some(FIRST_PAGE));
}

#[tokio::test]
async fn playback_is_silenced_while_the_closed_app_waits_for_streams() {
    let mut harness = streaming_harness().await;
    harness.audio.take();
    let _finished = request_shutdown(&mut harness);
    assert_eq!(harness.audio.take(), [AudioCall::Mute(true)]);
}

#[tokio::test]
async fn quitting_without_streams_leaves_the_sound_alone() {
    let mut harness = start_harness_with(test_settings(false, false), 10, None);
    harness.audio.take();
    let (done, _finished) = oneshot::channel();
    let ui = UiSettings::default();
    assert!(!harness.controller.handle(Command::Shutdown { ui, done }));
    assert!(!harness.audio.take().contains(&AudioCall::Mute(true)));
}

#[tokio::test]
async fn shutting_down_ends_background_work() {
    let mut harness = start_harness_with(test_settings(false, false), 10, None);
    let work = harness.session.background_work();
    assert!(harness.shutdown().await.is_ok());
    assert_eq!(work.run(std::future::pending::<()>()).await, None);
    assert!(harness.session.get().is_some());
}
