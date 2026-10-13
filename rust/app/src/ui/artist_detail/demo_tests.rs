use feishin_api::MusicServer;
use feishin_core::model::{ArtistDetail, Paging};

use super::actions::artist_songs_query;
use super::biography::plain_text;
use super::info::info_line;
use crate::demo::FakeServer;

async fn first_demo_artist_id(server: &FakeServer) -> String {
    server
        .album("album-0")
        .await
        .ok()
        .and_then(|album| {
            album
                .album
                .album_artists
                .first()
                .map(|artist| artist.id.clone())
        })
        .unwrap_or_default()
}

fn assert_detail_is_complete(detail: &ArtistDetail) {
    assert!(!detail.albums.is_empty());
    assert!(!detail.top_songs.is_empty());
    assert!(
        detail
            .top_songs
            .windows(2)
            .all(|pair| pair[0].play_count >= pair[1].play_count)
    );
    assert!(detail.artist.cover_art_id.is_some());
    assert!(!plain_text(detail.artist.biography.as_deref().unwrap_or_default()).is_empty());
    let info = info_line(detail);
    assert!(info.contains("album") && info.contains("songs"), "{info}");
}

#[tokio::test]
async fn demo_artist_detail_has_everything_the_view_shows() {
    let server = FakeServer::new(2000);
    let artist_id = first_demo_artist_id(&server).await;
    let Ok(detail) = server.artist(&artist_id).await else {
        panic!("demo artist {artist_id} missing");
    };
    assert_detail_is_complete(&detail);

    let query = artist_songs_query(&artist_id);
    let songs = server
        .song_list(
            &query,
            Paging {
                start_index: 0,
                limit: 500,
            },
        )
        .await
        .map(|page| page.items)
        .unwrap_or_default();
    let total: u32 = detail.albums.iter().map(|album| album.song_count).sum();
    assert_eq!(songs.len(), total as usize);
}
