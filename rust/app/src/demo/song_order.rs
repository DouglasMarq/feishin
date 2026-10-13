use std::sync::Arc;

use feishin_core::model::{SongListQuery, SongSort, SortOrder};

use super::catalog::{
    SONGS_PER_ALBUM, album_artist, album_name, album_of_song, album_year, artist_name, song_genre,
    song_is_favorite, song_title,
};
use super::hashing::scramble;
use super::ids::{album_id, artist_id, genre_id};
use super::server::FakeServer;

pub(super) struct CachedSongOrder {
    query: SongListQuery,
    order: Arc<Vec<usize>>,
}

impl FakeServer {
    pub(super) fn sorted_songs(&self, query: &SongListQuery) -> Arc<Vec<usize>> {
        if let Some(order) = self.cached_song_order(query) {
            return order;
        }
        let order = Arc::new(self.compute_song_order(query));
        self.remember_song_order(query, &order);
        order
    }

    fn cached_song_order(&self, query: &SongListQuery) -> Option<Arc<Vec<usize>>> {
        let cache = self.order_cache.lock().ok()?;
        cache
            .as_ref()
            .filter(|cached| cached.query == *query)
            .map(|cached| Arc::clone(&cached.order))
    }

    fn remember_song_order(&self, query: &SongListQuery, order: &Arc<Vec<usize>>) {
        if let Ok(mut cache) = self.order_cache.lock() {
            *cache = Some(CachedSongOrder {
                query: query.clone(),
                order: Arc::clone(order),
            });
        }
    }

    fn compute_song_order(&self, query: &SongListQuery) -> Vec<usize> {
        let search = normalized_search(query.search.as_deref());
        let mut order: Vec<usize> = (0..self.song_count)
            .filter(|&index| song_matches(index, query, search.as_deref()))
            .collect();
        self.sort_song_order(&mut order, query.sort);
        if query.order == SortOrder::Desc {
            order.reverse();
        }
        order
    }

    fn sort_song_order(&self, order: &mut Vec<usize>, sort: SongSort) {
        match sort {
            SongSort::Name => sort_by_text_key(order, song_title),
            SongSort::Album => sort_by_text_key(order, album_text_key),
            SongSort::Artist | SongSort::AlbumArtist => sort_by_text_key(order, artist_text_key),
            numeric_sort => {
                order.sort_by_key(|&index| (self.numeric_song_key(index, numeric_sort), index));
            }
        }
    }

    fn numeric_song_key(&self, index: usize, sort: SongSort) -> i64 {
        let song = self.song_at(index);
        match sort {
            SongSort::Duration => song.duration_ms as i64,
            SongSort::Year => i64::from(song.year.unwrap_or_default()),
            SongSort::PlayCount => i64::from(song.play_count),
            SongSort::Rating => i64::from(song.user_rating.unwrap_or_default()),
            SongSort::Favorited => i64::from(song.favorite),
            SongSort::RecentlyAdded => -(index as i64),
            SongSort::RecentlyPlayed => (scramble(index as u64 ^ 0x91a) % 10_000) as i64,
            SongSort::Random
            | SongSort::Name
            | SongSort::Album
            | SongSort::Artist
            | SongSort::AlbumArtist => scramble(index as u64 ^ 0x4a4d) as i64,
        }
    }
}

fn normalized_search(search: Option<&str>) -> Option<String> {
    search
        .map(str::trim)
        .filter(|search| !search.is_empty())
        .map(str::to_lowercase)
}

fn song_matches(index: usize, query: &SongListQuery, search: Option<&str>) -> bool {
    search.is_none_or(|search| song_matches_search(index, search))
        && song_passes_filters(index, query)
}

fn song_matches_search(index: usize, search: &str) -> bool {
    let album = album_of_song(index);
    song_title(index).to_lowercase().contains(search)
        || album_name(album).to_lowercase().contains(search)
        || artist_name(album_artist(album))
            .to_lowercase()
            .contains(search)
}

fn song_passes_filters(index: usize, query: &SongListQuery) -> bool {
    let album = album_of_song(index);
    let artist = album_artist(album);
    let year = album_year(album);
    let filters = [
        query
            .favorite
            .is_none_or(|favorite| favorite == song_is_favorite(index)),
        query.album_ids.is_empty() || query.album_ids.contains(&album_id(album)),
        query.artist_ids.is_empty() || query.artist_ids.contains(&artist_id(artist)),
        query.genre_ids.is_empty() || query.genre_ids.contains(&genre_id(song_genre(index))),
        query.year_from.is_none_or(|earliest| year >= earliest),
        query.year_to.is_none_or(|latest| year <= latest),
    ];
    filters.into_iter().all(|passes| passes)
}

fn album_text_key(index: usize) -> String {
    let album = album_of_song(index);
    format!(
        "{}\u{0}{album:05}\u{0}{:02}",
        album_name(album),
        index % SONGS_PER_ALBUM
    )
}

fn artist_text_key(index: usize) -> String {
    let album = album_of_song(index);
    format!(
        "{}\u{0}{}",
        artist_name(album_artist(album)),
        album_name(album)
    )
}

fn sort_by_text_key(order: &mut Vec<usize>, text_key: fn(usize) -> String) {
    let mut keyed: Vec<(String, usize)> = order
        .iter()
        .map(|&index| (text_key(index).to_lowercase(), index))
        .collect();
    keyed.sort();
    *order = keyed.into_iter().map(|(_, index)| index).collect();
}
