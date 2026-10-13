use std::time::{Duration, SystemTime, UNIX_EPOCH};

use feishin_api::{ApiError, MusicServer, StreamOptions};

use crate::support::{
    Backend, COVER_ALBUM, LYRICS_LINE, LYRICS_SONG, album_named, download, is_audio, is_image,
    song_titled,
};

fn now_ms() -> i64 {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock");
    i64::try_from(elapsed.as_millis()).expect("timestamp")
}

async fn play_count_reaches(server: &dyn MusicServer, id: &str, expected: u32) -> bool {
    for _ in 0..20 {
        if server.song(id).await.expect("song").play_count >= expected {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    false
}

pub async fn scrobbles_update_play_count(backend: Backend) {
    let server = backend.connect().await;
    let song = song_titled(&*server, "Noon Glare").await;
    let before = server.song(&song.id).await.expect("song").play_count;
    server
        .scrobble(&song.id, false, None)
        .await
        .expect("now playing");
    server
        .scrobble(&song.id, true, Some(now_ms()))
        .await
        .expect("submission");
    assert!(play_count_reaches(&*server, &song.id, before + 1).await);
}

pub async fn play_queue_round_trips(backend: Backend) {
    let server = backend.connect().await;
    let mut ids = Vec::new();
    for title in ["Undertow", "Dusk Signal", "Morning Haze"] {
        ids.push(song_titled(&*server, title).await.id);
    }
    let saved = server
        .save_play_queue(&ids, ids.get(1).map(String::as_str), 2_500)
        .await;
    if !backend.supports_play_queue() {
        assert!(matches!(saved, Err(ApiError::Unsupported(_))));
        return;
    }
    saved.expect("save play queue");
    let queue = server
        .play_queue()
        .await
        .expect("play queue")
        .expect("a saved queue");
    let restored: Vec<String> = queue.songs.into_iter().map(|song| song.id).collect();
    assert_eq!(restored, ids);
    assert_eq!(queue.current_song_id.as_ref(), ids.get(1));
    assert_eq!(queue.position_ms, 2_500);
}

pub async fn streams_return_audio(backend: Backend) {
    let server = backend.connect().await;
    for title in ["Dawn Chorus", LYRICS_SONG] {
        let song = song_titled(&*server, title).await;
        let original = download(&server.stream_url(&song.id, &StreamOptions::default())).await;
        assert!(
            is_audio(&original.bytes),
            "{title}: {}",
            original.content_type
        );
        let transcoded_options = StreamOptions {
            max_bitrate_kbps: Some(96),
            format: Some("mp3".into()),
        };
        let transcoded = download(&server.stream_url(&song.id, &transcoded_options)).await;
        assert!(
            is_audio(&transcoded.bytes),
            "{title} transcoded: {}",
            transcoded.content_type
        );
    }
}

pub async fn cover_art_downloads(backend: Backend) {
    let server = backend.connect().await;
    let album = album_named(&*server, COVER_ALBUM).await;
    let cover_id = album.cover_art_id.expect("album cover id");
    for edge in [None, Some(300)] {
        let cover = download(&server.cover_art_url(&cover_id, edge)).await;
        assert!(is_image(&cover.bytes), "{}", cover.content_type);
    }
}

pub async fn lyrics_are_found(backend: Backend) {
    let server = backend.connect().await;
    let song = song_titled(&*server, LYRICS_SONG).await;
    let lyrics = server
        .lyrics(&song)
        .await
        .expect("lyrics")
        .expect("lyrics present");
    assert!(lyrics.contains(LYRICS_LINE), "{lyrics}");
}
