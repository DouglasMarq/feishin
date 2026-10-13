use feishin_api::{MusicServer, Result};
use feishin_core::model::{Paging, Song, SongListQuery};

const FETCH_ALL_PAGE_SIZE: usize = 500;

pub async fn fetch_all_songs(server: &dyn MusicServer, query: &SongListQuery) -> Result<Vec<Song>> {
    let mut songs = Vec::new();
    loop {
        let paging = Paging {
            start_index: songs.len(),
            limit: FETCH_ALL_PAGE_SIZE,
        };
        let page = server.song_list(query, paging).await?;
        let received = page.items.len();
        songs.extend(page.items);
        let reached_total = page.total_count.is_some_and(|total| songs.len() >= total);
        if received < FETCH_ALL_PAGE_SIZE || reached_total {
            return Ok(songs);
        }
    }
}
