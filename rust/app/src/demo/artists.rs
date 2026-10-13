use std::cmp::Reverse;

use feishin_api::{ApiError, Result};
use feishin_core::model::{
    Artist, ArtistDetail, ArtistListQuery, ArtistSort, Page, Paging, Song, SortOrder,
};

use super::catalog::ARTIST_COUNT;
use super::hashing::scramble;
use super::ids::parse_artist_index;
use super::paging::paginate;
use super::server::FakeServer;

const TOP_SONG_LIMIT: usize = 20;

impl FakeServer {
    pub(super) fn artist_page(&self, query: &ArtistListQuery, paging: Paging) -> Page<Artist> {
        let search = query.search.as_deref().map(str::to_lowercase);
        let mut artists: Vec<Artist> = (0..ARTIST_COUNT)
            .map(|artist| self.artist_at(artist))
            .filter(|artist| artist_is_listed(artist, query, search.as_deref()))
            .collect();
        sort_artists(&mut artists, query.sort);
        if query.order == SortOrder::Desc {
            artists.reverse();
        }
        Page {
            total_count: None,
            ..paginate(artists.into_iter(), paging)
        }
    }

    pub(super) fn artist_detail(&self, id: &str) -> Result<ArtistDetail> {
        let artist = parse_artist_index(id)
            .filter(|&artist| artist < ARTIST_COUNT)
            .ok_or(ApiError::NotFound)?;
        Ok(ArtistDetail {
            artist: self.artist_at(artist),
            albums: self
                .artist_albums(artist)
                .map(|album| self.album_at(album))
                .collect(),
            top_songs: self.artist_top_songs(artist),
        })
    }

    fn artist_top_songs(&self, artist: usize) -> Vec<Song> {
        let mut top_songs: Vec<Song> = self
            .artist_albums(artist)
            .flat_map(|album| self.album_song_range(album))
            .map(|index| self.song_at(index))
            .collect();
        top_songs.sort_by_key(|song| Reverse(song.play_count));
        top_songs.truncate(TOP_SONG_LIMIT);
        top_songs
    }
}

fn artist_is_listed(artist: &Artist, query: &ArtistListQuery, search: Option<&str>) -> bool {
    query
        .favorite
        .is_none_or(|favorite| artist.favorite == favorite)
        && search.is_none_or(|search| artist.name.to_lowercase().contains(search))
}

fn sort_artists(artists: &mut [Artist], sort: ArtistSort) {
    match sort {
        ArtistSort::Name => artists.sort_by_key(|artist| artist.name.to_lowercase()),
        ArtistSort::AlbumCount => artists.sort_by_key(|artist| artist.album_count),
        ArtistSort::SongCount => artists.sort_by_key(|artist| artist.song_count),
        ArtistSort::Rating => artists.sort_by_key(|artist| artist.user_rating),
        ArtistSort::Favorited => artists.sort_by_key(|artist| artist.favorite),
        ArtistSort::PlayCount | ArtistSort::Random => {
            artists
                .sort_by_key(|artist| scramble(parse_artist_index(&artist.id).unwrap_or(0) as u64));
        }
    }
}
