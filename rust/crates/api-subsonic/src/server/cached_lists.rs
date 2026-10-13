use std::sync::Arc;

use feishin_api::Result;
use feishin_core::model::{Playlist, Song, SongListQuery};

use super::SubsonicServer;
use super::list_caches::{AlbumAndArtistIds, AllPlaylists, MusicFolderKey, StarredItems};
use crate::client::Params;
use crate::normalize;
use crate::types::{GetArtists, Playlists, Starred2, SubsonicArtist};

pub fn starred_songs_matching<'starred>(
    starred: &'starred StarredItems,
    query: &SongListQuery,
) -> Vec<&'starred Song> {
    starred
        .songs
        .iter()
        .filter(|song| {
            query.artist_ids.is_empty()
                || song
                    .album_artists
                    .iter()
                    .any(|artist| query.artist_ids.contains(&artist.id))
        })
        .collect()
}

impl SubsonicServer {
    pub(super) async fn album_and_artist_songs(
        &self,
        query: &SongListQuery,
    ) -> Result<Arc<Vec<Song>>> {
        self.list_caches
            .album_and_artist_songs
            .get_or_fetch(&AlbumAndArtistIds::of(query), || async {
                let songs = self
                    .songs_of_albums_and_artists(&query.album_ids, &query.artist_ids)
                    .await?;
                Ok(self.normalize_songs(songs))
            })
            .await
    }

    pub(super) async fn starred(&self, music_folder_id: Option<&str>) -> Result<Arc<StarredItems>> {
        self.list_caches
            .starred
            .get_or_fetch(&MusicFolderKey::new(music_folder_id), || {
                self.fetch_starred(music_folder_id)
            })
            .await
    }

    async fn fetch_starred(&self, music_folder_id: Option<&str>) -> Result<StarredItems> {
        let params = Params::new().add_optional("musicFolderId", music_folder_id);
        let response: Starred2 = self.fetch("getStarred2.view", params).await?;
        let starred = response.starred2.unwrap_or_default();
        Ok(StarredItems {
            songs: self.normalize_songs(starred.song.unwrap_or_default()),
            albums: self.normalize_albums(starred.album.unwrap_or_default()),
        })
    }

    pub(super) async fn artist_index(
        &self,
        music_folder_id: Option<&str>,
    ) -> Result<Arc<Vec<SubsonicArtist>>> {
        self.list_caches
            .artist_index
            .get_or_fetch(&MusicFolderKey::new(music_folder_id), || {
                self.fetch_artist_index(music_folder_id)
            })
            .await
    }

    async fn fetch_artist_index(
        &self,
        music_folder_id: Option<&str>,
    ) -> Result<Vec<SubsonicArtist>> {
        let params = Params::new().add_optional("musicFolderId", music_folder_id);
        let response: GetArtists = self.fetch("getArtists.view", params).await?;
        Ok(response
            .artists
            .and_then(|indexes| indexes.index)
            .unwrap_or_default()
            .into_iter()
            .flat_map(|index| index.artist.unwrap_or_default())
            .collect())
    }

    pub(super) async fn all_playlists(&self) -> Result<Arc<Vec<Playlist>>> {
        self.list_caches
            .playlists
            .get_or_fetch(&AllPlaylists, || self.fetch_all_playlists())
            .await
    }

    async fn fetch_all_playlists(&self) -> Result<Vec<Playlist>> {
        let response: Playlists = self.fetch("getPlaylists.view", Params::new()).await?;
        Ok(response
            .playlists
            .and_then(|container| container.playlist)
            .unwrap_or_default()
            .into_iter()
            .map(|playlist| normalize::playlist(playlist, self.server_id()))
            .collect())
    }
}
