use feishin_api::{Credential, MusicServer, StreamOptions};
use feishin_api_subsonic::SubsonicServer;

use crate::support::{CREDENTIAL, config, server_song};

#[test]
fn url_builders() {
    let credential = Credential::Subsonic {
        subsonic: CREDENTIAL.into(),
    };
    let server = SubsonicServer::with_credential(config("https://music.example.com/"), &credential)
        .expect("server");
    let mut song = server_song(&server);
    song.id = "a b/c".into();

    assert_eq!(
        server.stream_url(&song, &StreamOptions::default()),
        format!(
            "https://music.example.com/rest/stream.view?id=a%20b%2Fc&v=1.13.0&c=Feishin&{CREDENTIAL}"
        )
    );
    let options = StreamOptions {
        max_bitrate_kbps: Some(192),
        format: Some("opus".into()),
    };
    assert_eq!(
        server.stream_url(&song, &options),
        format!(
            "https://music.example.com/rest/stream.view?id=a%20b%2Fc&v=1.13.0&c=Feishin&{CREDENTIAL}&format=opus&maxBitRate=192"
        )
    );
    assert_eq!(
        server.cover_art_url("al-1", Some(300)),
        format!(
            "https://music.example.com/rest/getCoverArt.view?id=al-1&{CREDENTIAL}&v=1.13.0&c=Feishin&size=300"
        )
    );
    assert_eq!(
        server.cover_art_url("al-1", None),
        format!(
            "https://music.example.com/rest/getCoverArt.view?id=al-1&{CREDENTIAL}&v=1.13.0&c=Feishin"
        )
    );
}
