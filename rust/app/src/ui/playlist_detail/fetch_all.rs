use feishin_api::MusicServer;
use feishin_core::model::{Paging, Song};

const FETCH_ALL_PAGE_SIZE: usize = 500;

pub async fn fetch_all_playlist_songs(
    server: &dyn MusicServer,
    playlist_id: &str,
) -> feishin_api::Result<Vec<Song>> {
    let mut songs = Vec::new();
    loop {
        let paging = Paging {
            start_index: songs.len(),
            limit: FETCH_ALL_PAGE_SIZE,
        };
        let page = server.playlist_song_page(playlist_id, paging).await?;
        let received = page.items.len();
        songs.extend(page.items);
        if is_last_page(received, songs.len(), page.total_count) {
            return Ok(songs);
        }
    }
}

fn is_last_page(received: usize, collected: usize, total_count: Option<usize>) -> bool {
    match total_count {
        _ if received == 0 => true,
        Some(total) => collected >= total,
        None => received < FETCH_ALL_PAGE_SIZE,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::demo::FakeServer;

    #[test]
    fn short_pages_continue_until_the_reported_total() {
        assert!(!is_last_page(200, 200, Some(450)));
        assert!(is_last_page(250, 450, Some(450)));
        assert!(is_last_page(0, 200, Some(450)));
        assert!(!is_last_page(
            FETCH_ALL_PAGE_SIZE,
            FETCH_ALL_PAGE_SIZE,
            None
        ));
        assert!(is_last_page(10, 510, None));
    }

    #[tokio::test]
    async fn fetch_all_pages_through_the_whole_playlist() {
        let server: Arc<dyn MusicServer> = Arc::new(FakeServer::new(500));
        let expected = server
            .playlist_songs("playlist-1")
            .await
            .unwrap_or_default();
        let songs = fetch_all_playlist_songs(server.as_ref(), "playlist-1")
            .await
            .unwrap_or_default();
        assert!(!songs.is_empty());
        assert_eq!(songs, expected);
        assert!(
            fetch_all_playlist_songs(server.as_ref(), "missing")
                .await
                .is_err()
        );
    }
}
