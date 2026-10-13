use feishin_core::model::{Song, SongSort, SortOrder};

use super::key::{SortValue, created_day, number, order_by, text};
use super::shuffle;

fn title(song: &Song) -> SortValue {
    text(Some(&song.title))
}

fn album_name(song: &Song) -> SortValue {
    text(song.album.as_deref())
}

fn album_position(song: &Song) -> [SortValue; 3] {
    [
        album_name(song),
        number(song.disc_number.map(i64::from)),
        number(song.track_number.map(i64::from)),
    ]
}

fn then_album_position(leading: Vec<SortValue>, song: &Song) -> Vec<SortValue> {
    leading.into_iter().chain(album_position(song)).collect()
}

fn joined_artist_names(song: &Song) -> SortValue {
    let names: Vec<&str> = song
        .artists
        .iter()
        .map(|artist| artist.name.as_str())
        .collect();
    text(Some(&names.join(", ")))
}

fn first_album_artist(song: &Song) -> SortValue {
    text(
        song.album_artists
            .first()
            .map(|artist| artist.name.as_str()),
    )
}

fn song_sort_values(song: &Song, sort: SongSort) -> Vec<SortValue> {
    match sort {
        SongSort::Name => vec![title(song), album_name(song)],
        SongSort::Album => album_position(song).into(),
        SongSort::AlbumArtist => then_album_position(vec![first_album_artist(song)], song),
        SongSort::Artist => vec![
            joined_artist_names(song),
            number(song.disc_number.map(i64::from)),
            number(song.track_number.map(i64::from)),
        ],
        SongSort::Duration => {
            then_album_position(vec![number(i64::try_from(song.duration_ms).ok())], song)
        }
        SongSort::Year => then_album_position(vec![number(song.year.map(i64::from))], song),
        SongSort::PlayCount => {
            then_album_position(vec![number(Some(i64::from(song.play_count)))], song)
        }
        SongSort::Rating => then_album_position(
            vec![number(song.user_rating.map(i64::from)), title(song)],
            song,
        ),
        SongSort::Favorited => then_album_position(
            vec![number(Some(i64::from(song.favorite))), title(song)],
            song,
        ),
        SongSort::RecentlyAdded => {
            then_album_position(vec![created_day(song.created_at.as_deref())], song)
        }
        SongSort::RecentlyPlayed => {
            then_album_position(vec![SortValue::Text(song.last_played_at.clone())], song)
        }
        SongSort::Random => Vec::new(),
    }
}

pub fn sort_songs(items: &mut [Song], sort: SongSort, order: SortOrder) {
    let descending = order == SortOrder::Desc;
    match sort {
        SongSort::Random => shuffle(items),
        ordered => order_by(items, descending, |song| song_sort_values(song, ordered)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use feishin_core::model::ArtistRef;

    fn song(title: &str, album: Option<&str>, rating: Option<u8>) -> Song {
        Song {
            id: title.into(),
            server_id: "s".into(),
            title: title.into(),
            artists: vec![ArtistRef {
                id: "a".into(),
                name: "A".into(),
            }],
            album_artists: Vec::new(),
            album: album.map(Into::into),
            album_id: None,
            duration_ms: 0,
            track_number: Some(1),
            disc_number: Some(1),
            year: None,
            genres: Vec::new(),
            bitrate_kbps: None,
            play_count: 0,
            user_rating: rating,
            favorite: false,
            cover_art_id: None,
            replay_gain: None,
            path: None,
            size_bytes: None,
            container: None,
            created_at: None,
            last_played_at: None,
        }
    }

    fn titles(items: &[Song]) -> Vec<&str> {
        items.iter().map(|song| song.title.as_str()).collect()
    }

    #[test]
    fn name_sort_is_case_insensitive() {
        let mut items = vec![
            song("b", None, None),
            song("A", None, None),
            song("c", None, None),
        ];
        sort_songs(&mut items, SongSort::Name, SortOrder::Asc);
        assert_eq!(titles(&items), ["A", "b", "c"]);
        sort_songs(&mut items, SongSort::Name, SortOrder::Desc);
        assert_eq!(titles(&items), ["c", "b", "A"]);
    }

    #[test]
    fn missing_values_sort_last_ascending_first_descending() {
        let mut items = vec![
            song("x", None, None),
            song("y", None, Some(3)),
            song("z", None, Some(5)),
        ];
        sort_songs(&mut items, SongSort::Rating, SortOrder::Asc);
        assert_eq!(titles(&items), ["y", "z", "x"]);
        sort_songs(&mut items, SongSort::Rating, SortOrder::Desc);
        assert_eq!(titles(&items), ["x", "z", "y"]);
    }
}
