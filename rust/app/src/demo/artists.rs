use std::cmp::Reverse;

use feishin_api::{ApiError, Result};
use feishin_core::model::{
    Artist, ArtistDetail, ArtistListQuery, ArtistSort, Page, Paging, Song, SortOrder,
};

use super::catalog::{artist_name, rating_from_hash, song_play_count};
use super::hashing::scramble;
use super::ids::parse_artist_index;
use super::paging::paginate_order;
use super::records::{artist_is_favorite, artist_traits_hash};
use super::server::FakeServer;

const TOP_SONG_LIMIT: usize = 20;

impl FakeServer {
    pub(super) fn artist_page(&self, query: &ArtistListQuery, paging: Paging) -> Page<Artist> {
        let order = self.sorted_artists(query);
        Page {
            total_count: None,
            ..paginate_order(&order, paging, |artist| self.artist_at(artist))
        }
    }

    fn sorted_artists(&self, query: &ArtistListQuery) -> Vec<usize> {
        let search = query.search.as_deref().map(str::to_lowercase);
        let mut order: Vec<usize> = (0..self.library.artist_count())
            .filter(|&artist| artist_is_listed(artist, query, search.as_deref()))
            .collect();
        self.sort_artists(&mut order, query.sort);
        if query.order == SortOrder::Desc {
            order.reverse();
        }
        order
    }

    fn sort_artists(&self, order: &mut [usize], sort: ArtistSort) {
        match sort {
            ArtistSort::Name => {
                order.sort_by_cached_key(|&artist| artist_name(artist).to_lowercase());
            }
            ArtistSort::AlbumCount => order.sort_by_key(|&artist| self.artist_albums(artist).len()),
            ArtistSort::SongCount => order.sort_by_key(|&artist| self.artist_song_count(artist)),
            ArtistSort::Rating => {
                order.sort_by_key(|&artist| rating_from_hash(artist_traits_hash(artist)));
            }
            ArtistSort::Favorited => order.sort_by_key(|&artist| artist_is_favorite(artist)),
            ArtistSort::PlayCount | ArtistSort::Random => {
                order.sort_by_key(|&artist| scramble(artist as u64));
            }
        }
    }

    pub(super) fn artist_detail(&self, id: &str) -> Result<ArtistDetail> {
        let artist = parse_artist_index(id)
            .filter(|&artist| self.library.contains_artist(artist))
            .ok_or(ApiError::NotFound)?;
        Ok(ArtistDetail {
            artist: self.artist_at(artist),
            albums: self
                .artist_albums(artist)
                .iter()
                .map(|&album| self.album_at(album))
                .collect(),
            top_songs: self.artist_top_songs(artist),
        })
    }

    fn artist_top_songs(&self, artist: usize) -> Vec<Song> {
        let mut top_songs: Vec<usize> = self
            .artist_albums(artist)
            .iter()
            .flat_map(|&album| self.album_song_range(album))
            .collect();
        top_songs.sort_by_key(|&index| Reverse(song_play_count(index)));
        top_songs
            .into_iter()
            .take(TOP_SONG_LIMIT)
            .map(|index| self.song_at(index))
            .collect()
    }
}

fn artist_is_listed(artist: usize, query: &ArtistListQuery, search: Option<&str>) -> bool {
    query
        .favorite
        .is_none_or(|favorite| artist_is_favorite(artist) == favorite)
        && search.is_none_or(|search| artist_name(artist).to_lowercase().contains(search))
}
