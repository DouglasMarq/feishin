use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::Duration;

use feishin_core::queue::{QueueTrack, Uid, YouTubeTrack};
use feishin_storage::settings::Settings;
use tokio::sync::{Mutex, MutexGuard};

use super::harness::{Harness, start_harness, test_settings};
use crate::controller::{Command, TrackCommand};

static SPAWNING: Mutex<()> = Mutex::const_new(());

const CALL_POLL_INTERVAL: Duration = Duration::from_millis(10);
const CALL_WAIT_LIMIT: Duration = Duration::from_secs(5);

pub(super) const FUTURE_URL: &str =
    "https://rr1.googlevideo.example/videoplayback?expire=4102444800&id=1";
pub(super) const EXPIRED_URL: &str =
    "https://rr1.googlevideo.example/videoplayback?expire=1000&id=1";

pub(super) struct FakeYtDlp {
    directory: tempfile::TempDir,
    pub(super) program: PathBuf,
    _spawning: MutexGuard<'static, ()>,
}

impl FakeYtDlp {
    pub(super) async fn with_body(body: &str) -> Self {
        let spawning = SPAWNING.lock().await;
        let directory = tempfile::tempdir().unwrap();
        let program = directory.path().join("yt-dlp");
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$0.args\"\necho call >> \"$0.calls\"\n{body}\n"
        );
        std::fs::write(&program, script).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            directory,
            program,
            _spawning: spawning,
        }
    }

    pub(super) async fn printing(url: &str) -> Self {
        Self::with_body(&format!("echo '{url}'")).await
    }

    pub(super) async fn missing() -> Self {
        let fake = Self::with_body("true").await;
        Self {
            program: fake.directory.path().join("not-installed"),
            ..fake
        }
    }

    pub(super) fn calls(&self) -> usize {
        std::fs::read_to_string(self.directory.path().join("yt-dlp.calls"))
            .map(|calls| calls.lines().count())
            .unwrap_or(0)
    }

    pub(super) async fn wait_for_calls(&self, expected: usize) {
        let called = async {
            while self.calls() < expected {
                tokio::time::sleep(CALL_POLL_INTERVAL).await;
            }
        };
        tokio::time::timeout(CALL_WAIT_LIMIT, called).await.unwrap();
    }

    pub(super) fn last_arguments(&self) -> Vec<String> {
        std::fs::read_to_string(self.directory.path().join("yt-dlp.args"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    pub(super) fn settings(&self, is_gapless: bool, is_scrobbling_enabled: bool) -> Settings {
        let mut settings = test_settings(is_gapless, is_scrobbling_enabled);
        settings.youtube.yt_dlp_path = Some(self.program.to_string_lossy().into_owned());
        settings
    }

    pub(super) fn harness(&self, is_gapless: bool, is_scrobbling_enabled: bool) -> Harness {
        start_harness(self.settings(is_gapless, is_scrobbling_enabled))
    }
}

pub(super) fn youtube_track(video_id: &str) -> QueueTrack {
    QueueTrack::from(&YouTubeTrack {
        video_id: video_id.into(),
        title: format!("Video {video_id}"),
        artists: vec!["Singer".into()],
        album: None,
        thumbnail_url: Some(format!("https://i.ytimg.com/vi/{video_id}/hqdefault.jpg")),
        duration_ms: 200_000,
    })
}

pub(super) fn play_tracks(harness: &mut Harness, tracks: Vec<QueueTrack>) {
    harness
        .controller
        .handle(Command::Tracks(TrackCommand::PlayNow {
            tracks,
            start_index: Some(0),
        }));
}

pub(super) fn uid_of_video(harness: &Harness, video_id: &str) -> Uid {
    harness
        .uid_of(&format!("yt:{video_id}"))
        .unwrap_or(Uid::MAX)
}
