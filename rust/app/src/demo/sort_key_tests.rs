use feishin_api::MusicServer;
use feishin_core::model::{
    Artist, ArtistListQuery, ArtistSort, Paging, Song, SongListQuery, SongSort,
};

use super::FakeServer;
use super::ids::parse_song_index;

const LIBRARY_SONGS: usize = 3_000;

fn song_field(song: &Song, sort: SongSort) -> i64 {
    match sort {
        SongSort::Duration => song.duration_ms as i64,
        SongSort::Year => i64::from(song.year.unwrap_or_default()),
        SongSort::PlayCount => i64::from(song.play_count),
        SongSort::Rating => i64::from(song.user_rating.unwrap_or_default()),
        SongSort::Favorited => i64::from(song.favorite),
        _ => 0,
    }
}

fn artist_field(artist: &Artist, sort: ArtistSort) -> (String, i64) {
    let numeric = |value: Option<u32>| (String::new(), i64::from(value.unwrap_or_default()));
    match sort {
        ArtistSort::Name => (artist.name.to_lowercase(), 0),
        ArtistSort::AlbumCount => numeric(artist.album_count),
        ArtistSort::SongCount => numeric(artist.song_count),
        ArtistSort::Rating => numeric(artist.user_rating.map(u32::from)),
        ArtistSort::Favorited => numeric(Some(u32::from(artist.favorite))),
        ArtistSort::PlayCount | ArtistSort::Random => numeric(None),
    }
}

fn whole_library() -> Paging {
    Paging {
        start_index: 0,
        limit: LIBRARY_SONGS,
    }
}

#[tokio::test]
async fn numeric_song_sorts_follow_the_materialized_songs() {
    let server = FakeServer::new(LIBRARY_SONGS);
    let sorts = [
        SongSort::Duration,
        SongSort::Year,
        SongSort::PlayCount,
        SongSort::Rating,
        SongSort::Favorited,
    ];
    for sort in sorts {
        let query = SongListQuery {
            sort,
            ..SongListQuery::default()
        };
        let songs = server
            .song_list(&query, whole_library())
            .await
            .map(|page| page.items)
            .unwrap_or_default();
        assert_eq!(songs.len(), LIBRARY_SONGS);
        let keys: Vec<(i64, Option<usize>)> = songs
            .iter()
            .map(|song| (song_field(song, sort), parse_song_index(&song.id)))
            .collect();
        assert!(keys.is_sorted(), "{sort:?}");
    }
}

#[tokio::test]
async fn artist_sorts_follow_the_materialized_artists() {
    let server = FakeServer::new(LIBRARY_SONGS);
    let sorts = [
        ArtistSort::Name,
        ArtistSort::AlbumCount,
        ArtistSort::SongCount,
        ArtistSort::Rating,
        ArtistSort::Favorited,
    ];
    for sort in sorts {
        let query = ArtistListQuery {
            sort,
            ..ArtistListQuery::default()
        };
        let artists = server
            .artist_list(&query, whole_library())
            .await
            .map(|page| page.items)
            .unwrap_or_default();
        assert_eq!(artists.len(), 400);
        let keys: Vec<(String, i64)> = artists
            .iter()
            .map(|artist| artist_field(artist, sort))
            .collect();
        assert!(keys.is_sorted(), "{sort:?}");
    }
}
