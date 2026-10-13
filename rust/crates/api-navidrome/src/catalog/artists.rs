use feishin_api::{ApiError, Result};
use feishin_core::model::{
    AlbumListQuery, AlbumSort, Artist, ArtistDetail, ArtistListQuery, ArtistSort, Page, Paging,
    Song, SortOrder,
};

use crate::NavidromeServer;
use crate::catalog::paging::paginate;
use crate::features::{Features, OpenSubsonicExtension};
use crate::http::missing_session_error;
use crate::native::{NativeArtist, normalize_artist};
use crate::query::{self, QueryParams, end_index, end_index_or_all};
use crate::subsonic;

impl NavidromeServer {
    pub(crate) async fn list_artists(
        &self,
        query: &ArtistListQuery,
        paging: Paging,
    ) -> Result<Page<Artist>> {
        let features = self.features().await;
        self.native_or_subsonic(
            self.native_artist_page(query, paging, &features),
            self.subsonic_artist_page(query, paging),
        )
        .await
    }

    pub(crate) async fn fetch_artist_detail(&self, id: &str) -> Result<ArtistDetail> {
        let features = self.features().await;
        self.native_or_subsonic(
            self.native_artist(id, &features),
            self.subsonic_artist(id, &features),
        )
        .await
    }

    async fn native_artist_page(
        &self,
        query: &ArtistListQuery,
        paging: Paging,
        features: &Features,
    ) -> Result<Page<Artist>> {
        let start = paging.start_index;
        let params = query::artist_list(query, start, end_index(start, paging.limit), features);
        let (artists, total): (Vec<NativeArtist>, _) = self.native_get("artist", &params).await?;
        Ok(Page {
            items: artists
                .into_iter()
                .map(|artist| normalize_artist(artist, self.server_id()))
                .collect(),
            start_index: start,
            total_count: total,
        })
    }

    async fn subsonic_artist_page(
        &self,
        query: &ArtistListQuery,
        paging: Paging,
    ) -> Result<Page<Artist>> {
        if has_native_only_artist_filters(query) {
            return Err(missing_session_error());
        }
        let params: QueryParams = query
            .music_folder_id
            .clone()
            .map(|folder| ("musicFolderId", folder))
            .into_iter()
            .collect();
        let response: subsonic::Artists = self.subsonic("getArtists", &params).await?;
        let mut artists: Vec<Artist> = response
            .artists
            .unwrap_or_default()
            .index
            .into_iter()
            .flat_map(|index| index.artist)
            .map(|artist| subsonic::normalize_artist(artist, self.server_id()))
            .collect();
        if query.order == SortOrder::Desc {
            artists.reverse();
        }
        Ok(paginate(artists, paging))
    }

    async fn native_artist(&self, id: &str, features: &Features) -> Result<ArtistDetail> {
        let (artist, _): (NativeArtist, _) = self.native_get(&format!("artist/{id}"), &[]).await?;
        let albums = self
            .native_album_page(
                &artist_albums_query(id),
                0,
                end_index_or_all(0, 0),
                features,
            )
            .await?
            .items;
        let artist = normalize_artist(artist, self.server_id());
        let top_songs = self.top_songs(id, &artist.name, features).await?;
        Ok(ArtistDetail {
            artist,
            albums,
            top_songs,
        })
    }

    async fn subsonic_artist(&self, id: &str, features: &Features) -> Result<ArtistDetail> {
        let response: subsonic::ArtistResponse =
            self.subsonic("getArtist", &[("id", id.to_owned())]).await?;
        let mut artist = response.artist.ok_or(ApiError::NotFound)?;
        let albums = artist
            .album
            .take()
            .unwrap_or_default()
            .into_iter()
            .map(|album| subsonic::normalize_album(album, self.server_id()))
            .collect();
        let mut artist = subsonic::normalize_artist(artist, self.server_id());
        artist.biography = self.subsonic_biography(id).await;
        let top_songs = self.top_songs(id, &artist.name, features).await?;
        Ok(ArtistDetail {
            artist,
            albums,
            top_songs,
        })
    }

    async fn subsonic_biography(&self, id: &str) -> Option<String> {
        let response = self
            .subsonic::<subsonic::ArtistInfo2>("getArtistInfo2", &[("id", id.to_owned())])
            .await;
        match response {
            Ok(info) => info
                .artist_info2
                .and_then(|info| info.biography)
                .filter(|biography| !biography.is_empty()),
            Err(error) => {
                tracing::debug!(%error, artist_id = id, "artist biography unavailable");
                None
            }
        }
    }

    async fn top_songs(
        &self,
        artist_id: &str,
        artist_name: &str,
        features: &Features,
    ) -> Result<Vec<Song>> {
        let params = top_songs_params(artist_id, artist_name, features);
        match self
            .subsonic::<subsonic::TopSongs>("getTopSongs", &params)
            .await
        {
            Ok(response) => {
                Ok(self.songs_from_subsonic(response.top_songs.unwrap_or_default().song))
            }
            Err(error @ (ApiError::Server { .. } | ApiError::NotFound)) => {
                tracing::debug!(%error, artist_id, "top songs unavailable");
                Ok(Vec::new())
            }
            Err(error) => Err(error),
        }
    }
}

fn has_native_only_artist_filters(query: &ArtistListQuery) -> bool {
    query.favorite.is_some()
        || query
            .search
            .as_ref()
            .is_some_and(|search| !search.is_empty())
        || !matches!(query.sort, ArtistSort::Name)
}

fn artist_albums_query(artist_id: &str) -> AlbumListQuery {
    AlbumListQuery {
        sort: AlbumSort::Year,
        order: SortOrder::Desc,
        artist_ids: vec![artist_id.to_owned()],
        ..AlbumListQuery::default()
    }
}

fn top_songs_params(artist_id: &str, artist_name: &str, features: &Features) -> QueryParams {
    if features.has_extension(OpenSubsonicExtension::TopSongsByArtistId) {
        return vec![("id", artist_id.to_owned())];
    }
    vec![("artist", artist_name.to_owned())]
}
