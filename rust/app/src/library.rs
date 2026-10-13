use feishin_api::{MusicServer, Result};
use feishin_core::model::{Page, Paging, Song};

use crate::list_stream::{FolderWalk, SongSource, walk_page};

pub async fn song_page(
    server: &dyn MusicServer,
    source: &SongSource,
    paging: Paging,
) -> Result<Page<Song>> {
    match source {
        SongSource::Library(query) => server.song_list(query, paging).await,
        SongSource::Playlist(playlist_id) => {
            server.playlist_song_page(&playlist_id.0, paging).await
        }
        SongSource::Folder(folder) => server.folder_song_page(folder, paging).await,
        SongSource::FolderTree(root) => {
            walk_page(&mut FolderWalk::new(root.clone()), server, paging).await
        }
    }
}
