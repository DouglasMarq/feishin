use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use super::cookie_export::PrivateFile;
use super::*;
use crate::tests::SPAWNING;
use crate::{CookieBrowser, YtDlp};

const COOKIE_SCRIPT: &str = r#"while [ "$#" -gt 0 ]; do
  if [ "$1" = "--cookies" ]; then target="$2"; fi
  shift
done
stat -c %a "$target" > "$0.mode"
head -n 1 "$target" > "$0.header"
printf '# Netscape HTTP Cookie File\n.youtube.com\tTRUE\t/\tTRUE\t0\tSAPISID\tfixture-sapisid\n' > "$target"
"#;

struct FakeExporter {
    directory: tempfile::TempDir,
    program: PathBuf,
}

impl FakeExporter {
    fn with_body(body: &str) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let program = directory.path().join("yt-dlp");
        let script = format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$0.args\"\n{body}\n");
        std::fs::write(&program, script).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        Self { directory, program }
    }

    fn yt_dlp(&self) -> YtDlp {
        YtDlp::new(&self.program).with_cookies_from(Some(CookieBrowser::Firefox))
    }

    fn private_dir(&self) -> PathBuf {
        self.directory.path().join("runtime").join("feishin-rs")
    }

    fn recorded(&self, suffix: &str) -> String {
        std::fs::read_to_string(self.directory.path().join(format!("yt-dlp.{suffix}"))).unwrap()
    }
}

#[tokio::test]
async fn cookies_are_exported_to_a_private_file_and_deleted_after_reading() {
    let _spawning = SPAWNING.lock().await;
    let fake = FakeExporter::with_body(COOKIE_SCRIPT);
    let auth = sign_in_with_browser(&fake.yt_dlp(), &fake.private_dir())
        .await
        .unwrap();
    assert_eq!(auth.cookie_header(), "SAPISID=fixture-sapisid");
    assert_eq!(fake.recorded("mode").trim(), "600");
    assert_eq!(
        fake.recorded("header"),
        "# Netscape HTTP Cookie File\n",
        "yt-dlp refuses to load a cookie file without the Netscape header"
    );
    let mode = std::fs::metadata(fake.private_dir())
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o700);
    assert_eq!(std::fs::read_dir(fake.private_dir()).unwrap().count(), 0);
    let arguments: Vec<String> = fake.recorded("args").lines().map(str::to_owned).collect();
    let cookie_file = arguments[4].clone();
    assert!(cookie_file.starts_with(fake.private_dir().to_str().unwrap()));
    assert_eq!(
        arguments,
        [
            "--no-warnings",
            "--cookies-from-browser",
            "firefox",
            "--cookies",
            cookie_file.as_str(),
            "--quiet",
            "--skip-download",
            "--flat-playlist",
            "--playlist-items",
            "1",
            "--",
            "https://music.youtube.com/playlist?list=LM",
        ]
    );
}

#[tokio::test]
async fn yt_dlp_failures_without_cookies_explain_why() {
    let _spawning = SPAWNING.lock().await;
    let fake = FakeExporter::with_body(
        "echo 'ERROR: could not find firefox cookies database' >&2\nexit 1",
    );
    let error = sign_in_with_browser(&fake.yt_dlp(), &fake.private_dir())
        .await
        .unwrap_err();
    assert_eq!(
        error,
        MusicError::SignIn("yt-dlp failed: could not find firefox cookies database".into())
    );
    assert_eq!(std::fs::read_dir(fake.private_dir()).unwrap().count(), 0);
}

#[tokio::test]
async fn a_browser_without_a_youtube_session_is_reported() {
    let _spawning = SPAWNING.lock().await;
    let fake = FakeExporter::with_body("exit 0");
    let error = sign_in_with_browser(&fake.yt_dlp(), &fake.private_dir())
        .await
        .unwrap_err();
    assert_eq!(
        error,
        MusicError::SignIn("Firefox has no YouTube session".into())
    );
}

#[tokio::test]
async fn signing_in_needs_a_browser() {
    let fake = FakeExporter::with_body("exit 0");
    let without_browser = YtDlp::new(&fake.program);
    let error = sign_in_with_browser(&without_browser, &fake.private_dir())
        .await
        .unwrap_err();
    assert_eq!(error, MusicError::SignIn("no browser is selected".into()));
}

#[tokio::test]
#[ignore = "needs the real yt-dlp, network and FEISHIN_REAL_YTM=1"]
async fn the_real_yt_dlp_accepts_and_rewrites_the_prepared_cookie_file() {
    if std::env::var_os("FEISHIN_REAL_YTM").is_none() {
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    let cookie_file = PrivateFile::create(directory.path()).unwrap();
    let outcome = YtDlp::default().export_cookies(cookie_file.path()).await;
    let written = std::fs::read_to_string(cookie_file.path()).unwrap();
    assert!(
        !format!("{outcome:?}").contains("Netscape format"),
        "{outcome:?}"
    );
    assert!(written.contains(".youtube.com\t"), "{written}");
}
