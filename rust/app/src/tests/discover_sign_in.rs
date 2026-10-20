use feishin_storage::settings::YouTubeSettings;
use feishin_youtube::CookieBrowser;
use feishin_youtube::music::SystemLocale;
use serde_json::json;
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::youtube_support::FakeYtDlp;
use crate::discover::{DiscoverSource, FeedNotice, FeedRequest, SignIn, YouTubeMusicSource};

const HOME: &str = include_str!("../../../crates/youtube/src/music/fixtures/home.json");
const EXPORT_COOKIES: &str = r#"while [ "$#" -gt 0 ]; do
  if [ "$1" = "--cookies" ]; then target="$2"; fi
  shift
done
printf '.youtube.com\tTRUE\t/\tTRUE\t0\tSAPISID\tfixture-sapisid\n' > "$target""#;

fn home_response() -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_raw(HOME.as_bytes().to_vec(), "application/json")
}

fn settings(fake: &FakeYtDlp) -> YouTubeSettings {
    YouTubeSettings {
        yt_dlp_path: Some(fake.program.to_string_lossy().into_owned()),
        cookies_from_browser: Some(CookieBrowser::Chromium),
        country: None,
    }
}

fn source(server: &MockServer, private_dir: &tempfile::TempDir) -> YouTubeMusicSource {
    YouTubeMusicSource::new(
        SystemLocale::default(),
        &server.uri(),
        SignIn::new(Some(private_dir.path().join("feishin-rs"))),
    )
}

#[tokio::test]
async fn the_browser_session_personalizes_the_home_and_is_reused() {
    let fake = FakeYtDlp::with_body(EXPORT_COOKIES).await;
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/browse"))
        .and(header("cookie", "SAPISID=fixture-sapisid"))
        .and(body_partial_json(json!({"browseId": "FEmusic_home"})))
        .respond_with(home_response())
        .expect(2)
        .mount(&server)
        .await;
    let private_dir = tempfile::tempdir().unwrap();
    let discover = source(&server, &private_dir);
    let feed = discover
        .feed(FeedRequest::ForYou, settings(&fake))
        .await
        .unwrap();
    assert_eq!(
        feed.notice,
        FeedNotice::Personalized(CookieBrowser::Chromium)
    );
    assert_eq!(feed.shelves.len(), 2);
    discover
        .feed(FeedRequest::ForYou, settings(&fake))
        .await
        .unwrap();
    assert_eq!(fake.calls(), 1, "the sign-in is kept for the session");
    let leftovers = std::fs::read_dir(private_dir.path().join("feishin-rs"))
        .unwrap()
        .count();
    assert_eq!(leftovers, 0, "the cookie file is deleted");
}

#[tokio::test]
async fn a_rejected_session_falls_back_to_the_anonymous_home() {
    let fake = FakeYtDlp::with_body(EXPORT_COOKIES).await;
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(header("cookie", "SAPISID=fixture-sapisid"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(body_partial_json(json!({"browseId": "FEmusic_home"})))
        .respond_with(home_response())
        .mount(&server)
        .await;
    let private_dir = tempfile::tempdir().unwrap();
    let discover = source(&server, &private_dir);
    let feed = discover
        .feed(FeedRequest::ForYou, settings(&fake))
        .await
        .unwrap();
    assert!(feed.notice.is_warning());
    assert!(feed.notice.message().contains("401"));
    assert_eq!(feed.shelves.len(), 2);
    discover
        .feed(FeedRequest::ForYou, settings(&fake))
        .await
        .unwrap();
    assert_eq!(fake.calls(), 2, "a rejected session is signed in again");
}

#[tokio::test]
async fn a_browser_without_a_session_explains_the_fallback() {
    let fake =
        FakeYtDlp::with_body("echo 'ERROR: could not copy Chrome cookie database' >&2\nexit 1")
            .await;
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(home_response())
        .mount(&server)
        .await;
    let private_dir = tempfile::tempdir().unwrap();
    let feed = source(&server, &private_dir)
        .feed(FeedRequest::ForYou, settings(&fake))
        .await
        .unwrap();
    assert_eq!(
        feed.notice,
        FeedNotice::SignInFailed("yt-dlp failed: could not copy Chrome cookie database".into())
    );
}

#[tokio::test]
async fn pages_and_searches_do_not_wait_for_a_slow_sign_in() {
    let fake = FakeYtDlp::with_body(&format!("sleep 3\n{EXPORT_COOKIES}")).await;
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/browse"))
        .respond_with(home_response())
        .mount(&server)
        .await;
    let private_dir = tempfile::tempdir().unwrap();
    let discover = std::sync::Arc::new(source(&server, &private_dir));
    let signing_in = tokio::spawn({
        let discover = std::sync::Arc::clone(&discover);
        let settings = settings(&fake);
        async move { discover.feed(FeedRequest::ForYou, settings).await }
    });
    fake.wait_for_calls(1).await;
    let search = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        discover.search("anything".into(), settings(&fake)),
    )
    .await;
    assert_eq!(search, Ok(Ok(Vec::new())), "the search was not held back");
    let feed = signing_in.await.unwrap().unwrap();
    assert_eq!(
        feed.notice,
        FeedNotice::Personalized(CookieBrowser::Chromium)
    );
}
