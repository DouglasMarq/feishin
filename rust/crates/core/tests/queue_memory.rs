use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use feishin_core::model::{ArtistRef, ReplayGain, Song};
use feishin_core::queue::{Queue, Uid};
use serde::Serialize;

const SONG_COUNT: usize = 100_000;
const SMALL_SONG_COUNT: usize = 5_000;
const SONGS_PER_ALBUM: usize = 10;
const ARTIST_COUNT: usize = 2_000;
const MEBIBYTE: f64 = 1024.0 * 1024.0;

struct CountingAllocator;

static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        ALLOCATED_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATED_BYTES.fetch_add(new_size, Ordering::Relaxed);
        ALLOCATED_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.realloc(pointer, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

#[derive(Serialize)]
struct LegacyQueueItem<'song> {
    uid: Uid,
    song: &'song Song,
}

fn allocated_bytes() -> usize {
    ALLOCATED_BYTES.load(Ordering::Relaxed)
}

fn uuid_like(kind: &str, number: usize) -> String {
    format!("{kind:0>8}-{number:04x}-4c2a-9b1e-{:012x}", number * 7919)
}

fn artist(number: usize) -> ArtistRef {
    ArtistRef {
        id: uuid_like("artist", number % ARTIST_COUNT),
        name: format!("Artist Name {}", number % ARTIST_COUNT),
    }
}

fn realistic_song(number: usize) -> Song {
    let album_number = number / SONGS_PER_ALBUM;
    let album_id = uuid_like("album", album_number);
    Song {
        id: uuid_like("song", number),
        server_id: "5f0c9a52-7d43-4b8e-a1e2-31c6f9d0b7aa".into(),
        title: format!("A Realistic Song Title {number}"),
        artists: vec![artist(album_number), artist(album_number + 1)],
        album_artists: vec![artist(album_number)],
        album: Some(format!("The Album Called {album_number}")),
        album_id: Some(album_id.clone()),
        duration_ms: 215_000,
        track_number: Some((number % SONGS_PER_ALBUM) as u32 + 1),
        disc_number: Some(1),
        year: Some(2001),
        genres: vec!["Alternative Rock".into(), "Indie".into()],
        bitrate_kbps: Some(320),
        play_count: 4,
        user_rating: None,
        favorite: false,
        cover_art_id: Some(format!("al-{album_id}_66f1c0de")),
        replay_gain: Some(ReplayGain::default()),
        path: Some(format!(
            "/music/Artist Name {album_number}/The Album Called {album_number}/{number:02} - A Realistic Song Title {number}.flac"
        )),
        size_bytes: Some(31_457_280),
        container: Some("flac".into()),
        created_at: Some("2024-03-18T21:04:55.123456789Z".into()),
        last_played_at: Some("2025-11-02T08:15:42Z".into()),
    }
}

fn retained_by<Value>(build: impl FnOnce() -> Value) -> (Value, usize) {
    let before = allocated_bytes();
    let value = build();
    (value, allocated_bytes().saturating_sub(before))
}

fn legacy_json_bytes(songs: &[Song]) -> usize {
    let items: Vec<LegacyQueueItem<'_>> = songs
        .iter()
        .enumerate()
        .map(|(index, song)| LegacyQueueItem {
            uid: index as Uid + 1,
            song,
        })
        .collect();
    serde_json::to_vec(&items).map_or(0, |json| json.len())
}

fn compact_json_bytes(queue: &Queue) -> usize {
    serde_json::to_vec(&queue.snapshot().items).map_or(0, |json| json.len())
}

fn mebibytes(bytes: usize) -> f64 {
    bytes as f64 / MEBIBYTE
}

struct Measurement {
    full_bytes: usize,
    compact_bytes: usize,
    legacy_json_bytes: usize,
    compact_json_bytes: usize,
}

fn measure(song_count: usize) -> Measurement {
    let songs: Vec<Song> = (0..song_count).map(realistic_song).collect();
    let (full_items, full_bytes) = retained_by(|| {
        songs
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, song)| (index as Uid + 1, song))
            .collect::<Vec<(Uid, Song)>>()
    });
    drop(full_items);
    let (queue, compact_bytes) = retained_by(|| {
        let mut queue = Queue::with_seed(1);
        queue.play_now(songs.clone(), None);
        queue
    });
    assert_eq!(queue.len(), song_count);
    Measurement {
        full_bytes,
        compact_bytes,
        legacy_json_bytes: legacy_json_bytes(&songs),
        compact_json_bytes: compact_json_bytes(&queue),
    }
}

fn report(song_count: usize, measurement: &Measurement) {
    println!(
        "{song_count} queued songs: heap {:.1} MiB as full songs, {:.1} MiB as compact tracks; queue.json items {:.1} MiB -> {:.1} MiB",
        mebibytes(measurement.full_bytes),
        mebibytes(measurement.compact_bytes),
        mebibytes(measurement.legacy_json_bytes),
        mebibytes(measurement.compact_json_bytes)
    );
}

fn assert_compact(measurement: &Measurement) {
    assert!(measurement.compact_bytes * 3 < measurement.full_bytes);
    assert!(measurement.compact_json_bytes * 2 < measurement.legacy_json_bytes);
}

#[test]
fn compact_queue_retains_far_less_heap_than_full_songs() {
    let measurement = measure(SMALL_SONG_COUNT);
    report(SMALL_SONG_COUNT, &measurement);
    assert_compact(&measurement);
}

#[test]
#[ignore = "benchmark: run with --release -- --ignored --nocapture"]
fn benchmark_100k_queue_heap() {
    let measurement = measure(SONG_COUNT);
    report(SONG_COUNT, &measurement);
    assert_compact(&measurement);
}
