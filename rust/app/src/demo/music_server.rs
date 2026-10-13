use async_trait::async_trait;
use feishin_api::{ApiError, Credential, MusicServer, Result, StreamOptions};
use feishin_core::model::{
    Album, AlbumDetail, AlbumListQuery, Artist, ArtistDetail, ArtistListQuery, Genre, ItemKind,
    MusicFolder, Page, Paging, Playlist, SearchResults, ServerConfig, ServerPlayQueue, Song,
    SongListQuery,
};

use super::ids::parse_song_index;
use super::lyrics::demo_lyrics;
use super::paging::paginate_order;
use super::scrobbles::ScrobbleCall;
use super::server::FakeServer;

#[async_trait]
impl MusicServer for FakeServer {
    fn config(&self) -> &ServerConfig {
        &self.config
    }

    fn credential(&self) -> Option<Credential> {
        self.current_credential()
    }

    async fn ping(&self) -> Result<()> {
        Ok(())
    }

    async fn song_list(&self, query: &SongListQuery, paging: Paging) -> Result<Page<Song>> {
        let order = self.sorted_songs(query);
        Ok(paginate_order(&order, paging, |index| self.song_at(index)))
    }

    async fn song_count(&self, query: &SongListQuery) -> Result<usize> {
        Ok(self.sorted_songs(query).len())
    }

    async fn song(&self, id: &str) -> Result<Song> {
        self.song_index_in_library(id)
            .map(|index| self.song_at(index))
            .ok_or(ApiError::NotFound)
    }

    async fn random_songs(&self, count: usize, _genre: Option<&str>) -> Result<Vec<Song>> {
        Ok(self.random_selection(count))
    }

    async fn similar_songs(&self, song_id: &str, count: usize) -> Result<Vec<Song>> {
        let index = parse_song_index(song_id).ok_or(ApiError::NotFound)?;
        Ok(self.songs_similar_to(index, count))
    }

    async fn album_list(&self, query: &AlbumListQuery, paging: Paging) -> Result<Page<Album>> {
        let order = self.sorted_albums(query);
        Ok(paginate_order(&order, paging, |album| self.album_at(album)))
    }

    async fn album_count(&self, query: &AlbumListQuery) -> Result<usize> {
        Ok(self.sorted_albums(query).len())
    }

    async fn album(&self, id: &str) -> Result<AlbumDetail> {
        self.album_detail(id)
    }

    async fn artist_list(&self, query: &ArtistListQuery, paging: Paging) -> Result<Page<Artist>> {
        Ok(self.artist_page(query, paging))
    }

    async fn artist(&self, id: &str) -> Result<ArtistDetail> {
        self.artist_detail(id)
    }

    async fn genre_list(&self) -> Result<Vec<Genre>> {
        Ok(self.genres_with_counts())
    }

    async fn music_folders(&self) -> Result<Vec<MusicFolder>> {
        Ok(vec![MusicFolder {
            id: "folder-0".into(),
            name: "Music".into(),
        }])
    }

    async fn playlist_list(&self, paging: Paging) -> Result<Page<Playlist>> {
        Ok(self.playlist_page(paging))
    }

    async fn playlist_songs(&self, id: &str) -> Result<Vec<Song>> {
        self.songs_of_playlist(id)
    }

    async fn playlist(&self, id: &str) -> Result<Playlist> {
        self.playlist_by_id(id)
    }

    async fn playlist_song_page(&self, id: &str, paging: Paging) -> Result<Page<Song>> {
        self.playlist_songs_page(id, paging)
    }

    async fn create_playlist(&self, name: &str, song_ids: &[String]) -> Result<Playlist> {
        Ok(self.insert_playlist(name, song_ids))
    }

    async fn add_to_playlist(&self, playlist_id: &str, song_ids: &[String]) -> Result<()> {
        self.append_to_playlist(playlist_id, song_ids)
    }

    async fn remove_from_playlist(&self, playlist_id: &str, indexes: &[usize]) -> Result<()> {
        self.remove_playlist_positions(playlist_id, indexes)
    }

    async fn delete_playlist(&self, id: &str) -> Result<()> {
        self.remove_playlist(id)
    }

    async fn search(&self, query: &str, limit: usize) -> Result<SearchResults> {
        let song_query = SongListQuery {
            search: Some(query.to_owned()),
            ..SongListQuery::default()
        };
        let album_query = AlbumListQuery {
            search: song_query.search.clone(),
            ..AlbumListQuery::default()
        };
        let artist_query = ArtistListQuery {
            search: song_query.search.clone(),
            ..ArtistListQuery::default()
        };
        let paging = Paging {
            start_index: 0,
            limit,
        };
        Ok(SearchResults {
            songs: self.song_list(&song_query, paging).await?.items,
            albums: self.album_list(&album_query, paging).await?.items,
            artists: self.artist_list(&artist_query, paging).await?.items,
        })
    }

    async fn set_favorite(&self, _kind: ItemKind, _ids: &[String], _favorite: bool) -> Result<()> {
        Ok(())
    }

    async fn set_rating(&self, _kind: ItemKind, _id: &str, _rating: u8) -> Result<()> {
        Ok(())
    }

    async fn scrobble(
        &self,
        song_id: &str,
        submission: bool,
        timestamp_ms: Option<i64>,
    ) -> Result<()> {
        self.record_scrobble(ScrobbleCall {
            song_id: song_id.to_owned(),
            submission,
            timestamp_ms,
        });
        Ok(())
    }

    async fn save_play_queue(
        &self,
        song_ids: &[String],
        current_song_id: Option<&str>,
        position_ms: u64,
    ) -> Result<()> {
        self.store_play_queue(song_ids, current_song_id, position_ms);
        Ok(())
    }

    async fn play_queue(&self) -> Result<Option<ServerPlayQueue>> {
        Ok(self.stored_play_queue())
    }

    async fn lyrics(&self, song: &Song) -> Result<Option<String>> {
        Ok(parse_song_index(&song.id).map(|index| demo_lyrics(index, song.duration_ms)))
    }

    fn stream_url(&self, song_id: &str, _options: &StreamOptions) -> String {
        format!("demo://stream/{song_id}")
    }

    fn cover_art_url(&self, cover_art_id: &str, size: Option<u32>) -> String {
        format!("demo://cover/{cover_art_id}?size={}", size.unwrap_or(0))
    }
}
