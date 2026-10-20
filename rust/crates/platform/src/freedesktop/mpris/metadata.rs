use mpris_server::{Metadata, TrackId};
use url::Url;

use super::conversions::time_from_ms;
use crate::NowPlaying;
use crate::image_url::is_free_of_credentials;

const TRACK_PATH_PREFIX: &str = "/org/feishin/track/";
const REPLACEMENT_CHARACTER: char = '_';

pub(super) fn metadata_for(now_playing: &NowPlaying) -> Metadata {
    let mut metadata = Metadata::new();
    let Some(track_id) = track_id_for(&now_playing.track_id) else {
        return metadata;
    };
    metadata.set_trackid(Some(track_id));
    metadata.set_length(
        now_playing
            .duration_ms
            .filter(|duration_ms| *duration_ms > 0)
            .map(time_from_ms),
    );
    metadata.set_art_url(art_url(now_playing));
    metadata.set_album(non_empty(now_playing.album.as_ref()));
    metadata.set_artist((!now_playing.artists.is_empty()).then(|| now_playing.artists.clone()));
    metadata.set_title(non_empty(Some(&now_playing.title)));
    metadata
}

fn art_url(now_playing: &NowPlaying) -> Option<String> {
    non_empty(now_playing.cover_url.as_ref())
        .filter(|url| is_free_of_credentials(url))
        .or_else(|| {
            let cover_file = now_playing.cover_file.as_deref()?;
            Url::from_file_path(cover_file).ok().map(String::from)
        })
}

fn track_id_for(id: &str) -> Option<TrackId> {
    if id.is_empty() {
        return None;
    }
    let element: String = id.chars().map(object_path_character).collect();
    TrackId::try_from(format!("{TRACK_PATH_PREFIX}{element}")).ok()
}

fn object_path_character(character: char) -> char {
    match character {
        valid if valid.is_ascii_alphanumeric() || valid == '_' => valid,
        _ => REPLACEMENT_CHARACTER,
    }
}

fn non_empty(value: Option<&String>) -> Option<String> {
    value.filter(|text| !text.is_empty()).cloned()
}

#[cfg(test)]
mod tests {
    use mpris_server::Time;

    use super::*;

    fn song() -> NowPlaying {
        NowPlaying {
            track_id: "a1b2-c3d4.e5".to_owned(),
            title: "Song".to_owned(),
            artists: vec!["Artist A".to_owned(), "Artist B".to_owned()],
            album: Some("Album".to_owned()),
            duration_ms: Some(215_500),
            cover_url: Some("https://example.com/cover.jpg".to_owned()),
            cover_file: None,
        }
    }

    #[test]
    fn metadata_maps_all_fields() {
        let metadata = metadata_for(&song());
        assert_eq!(
            metadata.trackid().map(|track_id| track_id.to_string()),
            Some("/org/feishin/track/a1b2_c3d4_e5".to_owned())
        );
        assert_eq!(metadata.length(), Some(Time::from_millis(215_500)));
        assert_eq!(
            metadata.art_url(),
            Some("https://example.com/cover.jpg".to_owned())
        );
        assert_eq!(metadata.album(), Some("Album"));
        assert_eq!(
            metadata.artist(),
            Some(vec!["Artist A".to_owned(), "Artist B".to_owned()])
        );
        assert_eq!(metadata.title(), Some("Song"));
    }

    #[test]
    fn metadata_omits_empty_fields() {
        let metadata = metadata_for(&NowPlaying {
            track_id: "id".to_owned(),
            duration_ms: Some(0),
            album: Some(String::new()),
            ..NowPlaying::default()
        });
        assert!(metadata.trackid().is_some());
        assert_eq!(metadata.length(), None);
        assert_eq!(metadata.art_url(), None);
        assert_eq!(metadata.album(), None);
        assert_eq!(metadata.artist(), None);
        assert_eq!(metadata.title(), None);
    }

    #[test]
    fn given_a_cover_url_with_credentials_the_art_url_is_omitted() {
        let metadata = metadata_for(&NowPlaying {
            cover_url: Some(
                "http://navidrome.example/rest/getCoverArt.view?id=1&u=feishin&t=token&s=salt"
                    .to_owned(),
            ),
            ..song()
        });
        assert_eq!(metadata.art_url(), None);
        assert_eq!(metadata.title(), Some("Song"));
    }

    #[test]
    fn given_a_credential_cover_url_and_a_cached_cover_file_the_art_is_the_local_file() {
        let metadata = metadata_for(&NowPlaying {
            cover_url: Some(
                "http://navidrome.example/rest/getCoverArt.view?id=1&u=feishin&t=token&s=salt"
                    .to_owned(),
            ),
            cover_file: Some("/home/feishin/.cache/feishin-rs/covers/abc".into()),
            ..song()
        });
        assert_eq!(
            metadata.art_url().as_deref(),
            Some("file:///home/feishin/.cache/feishin-rs/covers/abc")
        );
    }

    #[test]
    fn given_a_cover_url_without_credentials_it_is_preferred_over_the_local_file() {
        let metadata = metadata_for(&NowPlaying {
            cover_url: Some("https://jellyfin.example/Items/1/Images/Primary".to_owned()),
            cover_file: Some("/home/feishin/.cache/feishin-rs/covers/abc".into()),
            ..song()
        });
        assert_eq!(
            metadata.art_url().as_deref(),
            Some("https://jellyfin.example/Items/1/Images/Primary")
        );
    }

    #[test]
    fn metadata_without_track_id_is_empty() {
        let metadata = metadata_for(&NowPlaying {
            title: "Song".to_owned(),
            ..NowPlaying::default()
        });
        assert!(metadata.trackid().is_none());
        assert_eq!(metadata.title(), None);
    }
}
