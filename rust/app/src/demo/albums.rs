use feishin_api::{ApiError, Result};
use feishin_core::model::AlbumDetail;

use super::ids::parse_album_index;
use super::server::FakeServer;

impl FakeServer {
    pub(super) fn album_detail(&self, id: &str) -> Result<AlbumDetail> {
        let album = parse_album_index(id)
            .filter(|&album| self.library.contains_album(album))
            .ok_or(ApiError::NotFound)?;
        Ok(AlbumDetail {
            album: self.album_at(album),
            songs: self
                .album_song_range(album)
                .map(|index| self.song_at(index))
                .collect(),
        })
    }
}
