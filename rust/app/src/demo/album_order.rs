use std::time::{SystemTime, UNIX_EPOCH};

use feishin_core::model::{AlbumListQuery, AlbumSort, SortOrder};

use super::catalog::{
    album_is_favorite, album_name, album_year, artist_name, first_song_of_album, song_duration,
    song_genre,
};
use super::hashing::scramble;
use super::ids::{artist_id, genre_id};
use super::server::FakeServer;

impl FakeServer {
    pub(super) fn sorted_albums(&self, query: &AlbumListQuery) -> Vec<usize> {
        let search = query.search.as_deref().map(str::to_lowercase);
        let mut order: Vec<usize> = (0..self.total_album_count())
            .filter(|&album| self.album_is_listed(album, query, search.as_deref()))
            .collect();
        let random_seed = random_seed();
        order.sort_by_cached_key(|&album| {
            (self.album_sort_key(album, query.sort, random_seed), album)
        });
        if query.order == SortOrder::Desc && query.sort != AlbumSort::Random {
            order.reverse();
        }
        order
    }

    fn album_is_listed(&self, album: usize, query: &AlbumListQuery, search: Option<&str>) -> bool {
        let artist = self.library.album_artist(album);
        search.is_none_or(|search| album_matches_search(album, artist, search))
            && query
                .favorite
                .is_none_or(|favorite| favorite == album_is_favorite(album))
            && (query.artist_ids.is_empty() || query.artist_ids.contains(&artist_id(artist)))
            && (query.genre_ids.is_empty()
                || query
                    .genre_ids
                    .contains(&genre_id(song_genre(first_song_of_album(album)))))
    }

    fn album_sort_key(&self, album: usize, sort: AlbumSort, random_seed: u64) -> (String, i64) {
        let numeric = |value: i64| (String::new(), value);
        match sort {
            AlbumSort::Name => (album_name(album).to_lowercase(), 0),
            AlbumSort::AlbumArtist => (artist_name(self.library.album_artist(album)), 0),
            AlbumSort::Year => numeric(i64::from(album_year(album))),
            AlbumSort::SongCount => numeric(self.album_song_range(album).len() as i64),
            AlbumSort::Duration => numeric(self.album_duration_ms(album) as i64),
            AlbumSort::Favorited => numeric(i64::from(album_is_favorite(album))),
            AlbumSort::RecentlyAdded => numeric(-(album as i64)),
            AlbumSort::RecentlyPlayed => numeric((scramble(album as u64 ^ 0x91a) % 10_000) as i64),
            AlbumSort::PlayCount => numeric((scramble(album as u64 ^ 0xc0) % 10_000) as i64),
            AlbumSort::Random => numeric((scramble(album as u64 ^ random_seed) % 10_000) as i64),
            AlbumSort::Rating => numeric(scramble(album as u64 ^ 0xa1) as i64),
        }
    }

    pub(super) fn album_duration_ms(&self, album: usize) -> u64 {
        self.album_song_range(album).map(song_duration).sum()
    }
}

fn album_matches_search(album: usize, artist: usize, search: &str) -> bool {
    album_name(album).to_lowercase().contains(search)
        || artist_name(artist).to_lowercase().contains(search)
}

fn random_seed() -> u64 {
    u64::from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos(),
    )
}
