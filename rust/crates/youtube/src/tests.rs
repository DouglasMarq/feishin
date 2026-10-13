use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::Duration;

use tokio::sync::Mutex;

use crate::links::songs_search_url;
use crate::search::parse_search_output;
use crate::*;

static SPAWNING: Mutex<()> = Mutex::const_new(());

const FUTURE_EXPIRY: u64 = 4_102_444_800;
const AUDIO_URL: &str = "https://rr1.googlevideo.example/videoplayback?expire=4102444800&id=7";

struct FakeYtDlp {
    directory: tempfile::TempDir,
    program: PathBuf,
}

impl FakeYtDlp {
    fn with_body(body: &str) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let program = directory.path().join("yt-dlp");
        let script = format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$0.args\"\n{body}\n");
        std::fs::write(&program, script).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        Self { directory, program }
    }

    fn printing(stdout: &str) -> Self {
        Self::with_body(&format!("cat <<'OUTPUT'\n{stdout}\nOUTPUT"))
    }

    fn yt_dlp(&self) -> YtDlp {
        YtDlp::new(&self.program)
    }

    fn arguments(&self) -> Vec<String> {
        let recorded = std::fs::read_to_string(self.directory.path().join("yt-dlp.args")).unwrap();
        recorded.lines().map(str::to_owned).collect()
    }
}

#[tokio::test]
async fn audio_stream_returns_the_printed_address_and_its_expiry() {
    let _spawning = SPAWNING.lock().await;
    let fake = FakeYtDlp::printing(AUDIO_URL);
    let stream = fake.yt_dlp().audio_stream("dQw4w9WgXcQ").await.unwrap();
    assert_eq!(stream.url, AUDIO_URL);
    assert_eq!(stream.expires_at, Some(FUTURE_EXPIRY));
    assert_eq!(
        fake.arguments(),
        [
            "--no-warnings",
            "--no-playlist",
            "-f",
            "bestaudio",
            "-g",
            "--",
            "https://music.youtube.com/watch?v=dQw4w9WgXcQ"
        ]
    );
}

#[tokio::test]
async fn cookies_come_from_the_chosen_browser() {
    let _spawning = SPAWNING.lock().await;
    let fake = FakeYtDlp::printing(AUDIO_URL);
    let yt_dlp = fake
        .yt_dlp()
        .with_cookies_from(Some(CookieBrowser::Firefox));
    yt_dlp.audio_stream("abc").await.unwrap();
    let arguments = fake.arguments();
    assert_eq!(arguments[1..3], ["--cookies-from-browser", "firefox"]);
}

#[tokio::test]
async fn a_failing_yt_dlp_reports_its_last_error_line() {
    let _spawning = SPAWNING.lock().await;
    let fake = FakeYtDlp::with_body(
        "echo 'WARNING: noise' >&2\necho 'ERROR: [youtube] abc: Video unavailable' >&2\nexit 1",
    );
    let error = fake.yt_dlp().audio_stream("abc").await.unwrap_err();
    assert_eq!(
        error,
        YouTubeError::Failed("[youtube] abc: Video unavailable".into())
    );
    assert!(error.user_message().contains("Video unavailable"));
}

#[tokio::test]
async fn output_without_an_address_is_an_error() {
    let _spawning = SPAWNING.lock().await;
    let fake = FakeYtDlp::printing("nothing useful");
    let error = fake.yt_dlp().audio_stream("abc").await.unwrap_err();
    assert_eq!(error, YouTubeError::NoAudioUrl);
}

#[tokio::test]
async fn a_missing_binary_asks_to_install_yt_dlp() {
    let directory = tempfile::tempdir().unwrap();
    let yt_dlp = YtDlp::new(directory.path().join("no-such-yt-dlp"));
    let error = yt_dlp.audio_stream("abc").await.unwrap_err();
    assert_eq!(error, YouTubeError::NotInstalled);
    assert_eq!(
        error.user_message(),
        "Install yt-dlp to play YouTube tracks"
    );
}

#[tokio::test]
async fn a_hanging_yt_dlp_times_out() {
    let _spawning = SPAWNING.lock().await;
    let fake = FakeYtDlp::with_body("sleep 5");
    let yt_dlp = fake.yt_dlp().with_timeout(Duration::from_millis(200));
    let error = yt_dlp.audio_stream("abc").await.unwrap_err();
    assert_eq!(error, YouTubeError::TimedOut(0));
}

#[tokio::test]
async fn video_ids_that_could_be_options_are_refused() {
    let error = YtDlp::default()
        .audio_stream("--exec=rm")
        .await
        .unwrap_err();
    assert_eq!(error, YouTubeError::InvalidVideoId("--exec=rm".into()));
}

#[tokio::test]
async fn search_lists_songs_from_the_flat_playlist() {
    let _spawning = SPAWNING.lock().await;
    let lines = [
        r#"{"id":"wU26xVT_vBU","title":"One More Time","url":"https://music.youtube.com/watch?v=wU26xVT_vBU"}"#,
        r#"{"id":"AIKwmk4mnNw","title":"Around the World","duration":421.5,"channel":"Daft Punk"}"#,
    ];
    let fake = FakeYtDlp::printing(&lines.join("\n"));
    let hits = fake.yt_dlp().search_songs("daft punk", 2).await.unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].video_id, "wU26xVT_vBU");
    assert_eq!(hits[1].artist.as_deref(), Some("Daft Punk"));
    assert_eq!(hits[1].duration_ms, Some(421_500));
    let arguments = fake.arguments();
    assert_eq!(arguments[arguments.len() - 3..][0], "1:2");
    assert_eq!(
        arguments.last().unwrap(),
        "https://music.youtube.com/search?q=daft%20punk#songs"
    );
}

#[test]
fn search_output_skips_broken_lines_and_bad_ids() {
    let output = "not json\n{\"id\":\"a b\",\"title\":\"Bad\"}\n\n{\"id\":\"ok_1\",\"title\":\"Fine\",\"artists\":[\"A\",\"B\"]}";
    let hits = parse_search_output(output);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].artist.as_deref(), Some("A, B"));
    assert_eq!(hits[0].duration_ms, None);
}

#[test]
fn search_addresses_escape_the_query() {
    assert_eq!(
        songs_search_url(" AC/DC & friends "),
        "https://music.youtube.com/search?q=AC%2FDC%20%26%20friends#songs"
    );
}

#[test]
fn stream_urls_read_their_expiry() {
    assert_eq!(
        StreamUrl::new(AUDIO_URL.into()).expires_at,
        Some(FUTURE_EXPIRY)
    );
    assert_eq!(StreamUrl::new("https://host/a?b=1".into()).expires_at, None);
    assert_eq!(
        StreamUrl::new("https://host/a?expire=x".into()).expires_at,
        None
    );
}

#[test]
fn cached_streams_expire_before_their_deadline() {
    let mut cache = StreamCache::new(4);
    let stream = StreamUrl::new("https://host/a?expire=10000".into());
    cache.insert("video", stream, 1_000);
    assert!(cache.get("video", 9_000).is_some());
    assert_eq!(cache.get("video", 9_500), None);
    assert!(cache.is_empty());
}

#[test]
fn already_expired_streams_are_not_cached() {
    let mut cache = StreamCache::new(4);
    cache.insert(
        "video",
        StreamUrl::new("https://host/a?expire=100".into()),
        1_000,
    );
    assert!(cache.is_empty());
}

#[test]
fn streams_without_an_expiry_live_for_an_hour() {
    let mut cache = StreamCache::new(4);
    cache.insert("video", StreamUrl::new("https://host/a".into()), 1_000);
    assert!(cache.get("video", 4_000).is_some());
    assert_eq!(cache.get("video", 4_600), None);
}

#[test]
fn the_cache_keeps_only_the_newest_streams() {
    let mut cache = StreamCache::new(2);
    for video_id in ["one", "two", "three"] {
        cache.insert(
            video_id,
            StreamUrl::new(format!("https://host/{video_id}")),
            0,
        );
    }
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.get("one", 0), None);
    assert_eq!(cache.get("three", 0).as_deref(), Some("https://host/three"));
    cache.remove("three");
    assert_eq!(cache.get("three", 0), None);
}

#[test]
fn every_browser_has_a_yt_dlp_name() {
    let names: Vec<_> = CookieBrowser::ALL
        .iter()
        .map(|browser| browser.yt_dlp_name())
        .collect();
    assert_eq!(
        names,
        [
            "firefox", "chrome", "chromium", "brave", "vivaldi", "edge", "opera", "whale"
        ]
    );
}

#[test]
fn video_ids_and_links() {
    assert!(is_video_id("dQw4w9WgXcQ"));
    assert!(!is_video_id(""));
    assert!(!is_video_id("a b"));
    assert_eq!(
        thumbnail_url("abc"),
        "https://i.ytimg.com/vi/abc/hqdefault.jpg"
    );
}
