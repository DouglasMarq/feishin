use feishin_api::MusicServer;
use feishin_core::model::{Paging, Playlist};

const PAGE_SIZE: usize = 500;

pub async fn fetch_all(server: &dyn MusicServer) -> feishin_api::Result<Vec<Playlist>> {
    let mut all = Vec::new();
    loop {
        let paging = Paging {
            start_index: all.len(),
            limit: PAGE_SIZE,
        };
        let page = server.playlist_list(paging).await?;
        let count = page.items.len();
        all.extend(page.items);
        if count < PAGE_SIZE || page.total_count.is_some_and(|total| all.len() >= total) {
            return Ok(all);
        }
    }
}

pub fn song_count_label(count: usize) -> String {
    match count {
        1 => "1 song".to_owned(),
        count => format!("{count} songs"),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::demo::FakeServer;

    #[test]
    fn song_counts_read_naturally() {
        assert_eq!(song_count_label(1), "1 song");
        assert_eq!(song_count_label(4), "4 songs");
    }

    #[tokio::test]
    async fn fetch_all_reads_the_demo_playlists() {
        let server: Arc<dyn MusicServer> = Arc::new(FakeServer::new(500));
        let playlists = fetch_all(server.as_ref()).await.unwrap_or_default();
        assert!(!playlists.is_empty());
        assert!(playlists.iter().all(|playlist| playlist.song_count > 0));
    }
}
