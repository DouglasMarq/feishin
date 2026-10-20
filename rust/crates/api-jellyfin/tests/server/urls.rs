use feishin_api::{ApiError, Credential, MusicServer, StreamOptions};
use feishin_api_jellyfin::JellyfinServer;
use reqwest::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::{TOKEN, USER, authed, config, server, server_at};

const SERVER_URL: &str = "https://jf.example";

#[test]
fn stream_url_downloads_original_without_options() {
    let jellyfin = server_at(SERVER_URL);
    assert_eq!(
        jellyfin.stream_url("s1", &StreamOptions::default()),
        "https://jf.example/Items/s1/Download?apiKey=tok123&playSessionId="
    );
}

#[test]
fn stream_url_transcodes_to_requested_format_and_bitrate() {
    let jellyfin = server_at(SERVER_URL);
    let options = StreamOptions {
        max_bitrate_kbps: Some(192),
        format: Some("opus".into()),
    };
    assert_eq!(
        jellyfin.stream_url("s1", &options),
        "https://jf.example/audio/s1/universal?userId=u1&deviceId=&audioCodec=opus&apiKey=tok123\
         &playSessionId=feishin-s1-opus-0-0&container=opus&transcodingProtocol=http\
         &transcodingContainer=opus&maxStreamingBitrate=192000"
    );
}

#[test]
fn stream_url_transcodes_to_mp3_when_only_bitrate_is_set() {
    let jellyfin = server_at(SERVER_URL);
    let options = StreamOptions {
        max_bitrate_kbps: Some(128),
        format: None,
    };
    assert_eq!(
        jellyfin.stream_url("s1", &options),
        "https://jf.example/audio/s1/universal?userId=u1&deviceId=&audioCodec=mp3&apiKey=tok123\
         &playSessionId=feishin-s1-mp3-0-0&container=mp3&transcodingProtocol=http\
         &transcodingContainer=mp3&maxStreamingBitrate=128000"
    );
}

#[test]
fn cover_art_url_adds_size_only_when_requested() {
    let jellyfin = server_at(SERVER_URL);
    assert_eq!(
        jellyfin.cover_art_url("al1", Some(300)),
        "https://jf.example/Items/al1/Images/Primary?quality=96&fillHeight=300&fillWidth=300"
    );
    assert_eq!(
        jellyfin.cover_art_url("al1", None),
        "https://jf.example/Items/al1/Images/Primary?quality=96"
    );
}

#[test]
fn given_a_transcode_format_with_query_characters_it_is_encoded() {
    let jellyfin = server_at(SERVER_URL);
    let options = StreamOptions {
        max_bitrate_kbps: None,
        format: Some("mp3&apiKey=stolen".into()),
    };

    let url = Url::parse(&jellyfin.stream_url("s1", &options)).unwrap();

    let values_of = |wanted: &str| -> Vec<String> {
        url.query_pairs()
            .filter(|(key, _)| key == wanted)
            .map(|(_, value)| value.into_owned())
            .collect()
    };
    assert_eq!(values_of("apiKey"), ["tok123"]);
    assert_eq!(values_of("audioCodec"), ["mp3&apiKey=stolen"]);
}

#[test]
fn given_an_id_with_reserved_characters_the_path_is_encoded() {
    let jellyfin = server_at(SERVER_URL);
    assert_eq!(
        jellyfin.cover_art_url("al/1?x#y", None),
        "https://jf.example/Items/al%2F1%3Fx%23y/Images/Primary?quality=96"
    );
    assert_eq!(
        jellyfin.stream_url("s/1", &StreamOptions::default()),
        "https://jf.example/Items/s%2F1/Download?apiKey=tok123&playSessionId="
    );
}

#[tokio::test]
async fn given_an_id_with_reserved_characters_the_api_path_is_encoded() {
    let mock = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/items/pl%2F1%3Fx%23y"))
        .and(authed())
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&mock)
        .await;

    server(&mock).delete_playlist("pl/1?x#y").await.unwrap();
}

#[test]
fn given_an_invalid_server_address_the_client_is_not_created() {
    let credential = Credential::Jellyfin {
        access_token: TOKEN.into(),
        user_id: USER.into(),
    };
    for address in ["not a url", "mailto:feishin@jf.example"] {
        let created = JellyfinServer::with_credential(config(address), &credential);
        assert!(
            matches!(created, Err(ApiError::InvalidAddress(_))),
            "{address}"
        );
    }
}

#[tokio::test]
async fn given_a_server_under_a_sub_path_requests_keep_the_prefix() {
    let mock = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/jellyfin/items/pl1"))
        .and(authed())
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&mock)
        .await;

    server_at(&format!("{}/jellyfin", mock.uri()))
        .delete_playlist("pl1")
        .await
        .unwrap();
}
