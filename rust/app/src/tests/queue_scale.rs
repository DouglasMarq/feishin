use std::cell::Cell;
use std::time::{Duration, Instant};

use feishin_core::model::{ArtistRef, Song};
use feishin_core::queue::Uid;

use super::harness::{Harness, start_harness, test_settings};
use crate::controller::{Command, UiUpdate};
use crate::demo::test_song;

const SONG_COUNT: usize = 100_000;
const EDIT_ROUNDS: u32 = 10;

fn realistic_song(number: usize) -> Song {
    let album_number = number / 10;
    let artist = |artist_number: usize| ArtistRef {
        id: format!("artist-{artist_number}"),
        name: format!("Artist Name {artist_number}"),
    };
    Song {
        title: format!("A Realistic Song Title {number}"),
        artists: vec![
            artist(album_number % 2_000),
            artist((album_number + 1) % 2_000),
        ],
        album: Some(format!("The Album Called {album_number}")),
        album_id: Some(format!("album-{album_number}")),
        cover_art_id: Some(format!("al-{album_number}")),
        ..test_song(&format!("song-{number}"), 215_000)
    }
}

fn timed(harness: &mut Harness, command: impl Fn(&Harness) -> Command) -> Duration {
    let started = Instant::now();
    for _ in 0..EDIT_ROUNDS {
        let next_command = command(harness);
        harness.controller.handle(next_command);
    }
    started.elapsed() / EDIT_ROUNDS
}

fn middle_uid(harness: &Harness) -> Uid {
    let order = harness.controller.queue().display_order();
    order.get(order.len() / 2).map_or(Uid::MAX, |item| item.uid)
}

fn panel_bytes(harness: &Harness) -> usize {
    let bytes = Cell::new(0);
    harness.ui.any(|update| {
        if let UiUpdate::Queue(view) = update {
            bytes.set(view.retained_bytes());
        }
        false
    });
    bytes.get()
}

#[tokio::test]
#[ignore = "benchmark: run with --release -- --ignored --nocapture"]
async fn benchmark_100k_queue_publishing() {
    let mut harness = start_harness(test_settings(false, false));
    let songs: Vec<Song> = (0..SONG_COUNT).map(realistic_song).collect();
    let started = Instant::now();
    harness.controller.play_now(songs, "song-0");
    let play_now = started.elapsed();
    harness.ui.clear();
    let remove = timed(&mut harness, |harness| {
        Command::Remove(vec![middle_uid(harness)])
    });
    let reorder = timed(&mut harness, |harness| Command::Move {
        uids: vec![middle_uid(harness)],
        before: harness.current_uid(),
    });
    let add_next = timed(&mut harness, |_| {
        Command::AddNext(vec![realistic_song(SONG_COUNT + 1)])
    });
    let track_change = timed(&mut harness, |_| Command::Next);
    let shuffle = timed(&mut harness, |_| Command::ToggleShuffle);
    let shuffled_track_change = timed(&mut harness, |_| Command::Next);
    let retained = panel_bytes(&harness);
    println!(
        "{SONG_COUNT} queued: play_now {play_now:?}; per edit: remove {remove:?}, move {reorder:?}, add next {add_next:?}, shuffle toggle {shuffle:?}, track change {track_change:?}, shuffled track change {shuffled_track_change:?}; panel retains {:.1} MiB",
        retained as f64 / (1024.0 * 1024.0)
    );
}
