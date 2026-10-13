//! Client-side sorting used where Subsonic cannot sort (port of `sortSongList`, `sortAlbumList` and
//! `sortAlbumArtistList` in `src/shared/api/utils.ts`). Missing values sort last ascending, like lodash.

use std::cmp::Ordering;

use feishin_core::model::{Album, AlbumSort, ArtistSort, Song, SongSort, SortOrder};
use rand::seq::SliceRandom;

use crate::normalize::truthy;
use crate::types::SsArtist;

#[derive(PartialEq, Eq)]
enum Val {
    S(Option<String>),
    N(Option<i64>),
}

struct Part(Val, bool);

struct Key(Vec<Part>);

fn cmp_opt<T: Ord>(a: &Option<T>, b: &Option<T>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => a.cmp(b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        for (Part(a, desc), Part(b, _)) in self.0.iter().zip(&other.0) {
            let ord = match (a, b) {
                (Val::S(a), Val::S(b)) => cmp_opt(a, b),
                (Val::N(a), Val::N(b)) => cmp_opt(a, b),
                _ => Ordering::Equal,
            };
            let ord = if *desc { ord.reverse() } else { ord };
            if ord != Ordering::Equal {
                return ord;
            }
        }
        Ordering::Equal
    }
}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Key {}

fn s(value: Option<&str>) -> Val {
    Val::S(value.map(str::to_lowercase))
}

fn n(value: Option<i64>) -> Val {
    Val::N(value)
}

fn order_by<T>(items: &mut [T], desc: bool, key: impl Fn(&T) -> Vec<Val>) {
    items.sort_by_cached_key(|item| Key(key(item).into_iter().map(|v| Part(v, desc)).collect()));
}

pub fn shuffle<T>(items: &mut [T]) {
    items.shuffle(&mut rand::rng());
}

fn day(created_at: Option<&str>) -> Val {
    Val::S(created_at.map(|c| c.chars().take(10).collect()))
}

pub fn songs(items: &mut [Song], sort: SongSort, order: SortOrder) {
    let desc = order == SortOrder::Desc;
    let album = |x: &Song| s(x.album.as_deref());
    let disc = |x: &Song| n(x.disc_number.map(i64::from));
    let track = |x: &Song| n(x.track_number.map(i64::from));
    let name = |x: &Song| s(Some(&x.title));
    match sort {
        SongSort::Name => order_by(items, desc, |x| vec![name(x), album(x)]),
        SongSort::Album => order_by(items, desc, |x| vec![album(x), disc(x), track(x)]),
        SongSort::AlbumArtist => order_by(items, desc, |x| {
            vec![
                s(x.album_artists.first().map(|a| a.name.as_str())),
                album(x),
                disc(x),
                track(x),
            ]
        }),
        SongSort::Artist => order_by(items, desc, |x| {
            let names: Vec<&str> = x.artists.iter().map(|a| a.name.as_str()).collect();
            vec![s(Some(&names.join(", "))), disc(x), track(x)]
        }),
        SongSort::Duration => order_by(items, desc, |x| {
            vec![
                n(i64::try_from(x.duration_ms).ok()),
                album(x),
                disc(x),
                track(x),
            ]
        }),
        SongSort::Year => order_by(items, desc, |x| {
            vec![n(x.year.map(i64::from)), album(x), disc(x), track(x)]
        }),
        SongSort::PlayCount => order_by(items, desc, |x| {
            vec![
                n(Some(i64::from(x.play_count))),
                album(x),
                disc(x),
                track(x),
            ]
        }),
        SongSort::Rating => order_by(items, desc, |x| {
            vec![
                n(x.user_rating.map(i64::from)),
                name(x),
                album(x),
                disc(x),
                track(x),
            ]
        }),
        SongSort::Favorited => order_by(items, desc, |x| {
            vec![
                n(Some(i64::from(x.favorite))),
                name(x),
                album(x),
                disc(x),
                track(x),
            ]
        }),
        SongSort::RecentlyAdded => order_by(items, desc, |x| {
            vec![day(x.created_at.as_deref()), album(x), disc(x), track(x)]
        }),
        SongSort::RecentlyPlayed => order_by(items, desc, |x| {
            vec![
                Val::S(x.last_played_at.clone()),
                album(x),
                disc(x),
                track(x),
            ]
        }),
        SongSort::Random => shuffle(items),
    }
}

pub fn albums(items: &mut [Album], sort: AlbumSort, order: SortOrder) {
    let desc = order == SortOrder::Desc;
    match sort {
        AlbumSort::Name => order_by(items, desc, |x| vec![s(Some(&x.name))]),
        AlbumSort::AlbumArtist => {
            order_by(items, false, |x| vec![s(Some(&x.name))]);
            order_by(items, desc, |x| {
                vec![s(x.album_artists.first().map(|a| a.name.as_str()))]
            });
        }
        AlbumSort::Year => order_by(items, desc, |x| vec![n(x.year.map(i64::from))]),
        AlbumSort::PlayCount => order_by(items, desc, |x| vec![n(Some(i64::from(x.play_count)))]),
        AlbumSort::Rating => order_by(items, desc, |x| vec![n(x.user_rating.map(i64::from))]),
        AlbumSort::Favorited => order_by(items, desc, |x| vec![n(Some(i64::from(x.favorite)))]),
        AlbumSort::RecentlyAdded => order_by(items, desc, |x| vec![Val::S(x.created_at.clone())]),
        AlbumSort::RecentlyPlayed => {
            order_by(items, desc, |x| vec![Val::S(x.last_played_at.clone())])
        }
        AlbumSort::SongCount => order_by(items, desc, |x| vec![n(Some(i64::from(x.song_count)))]),
        AlbumSort::Duration => {
            order_by(items, desc, |x| vec![n(i64::try_from(x.duration_ms).ok())])
        }
        AlbumSort::Random => shuffle(items),
    }
}

fn starred(value: &SsArtist) -> Option<String> {
    match &value.starred {
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        other => truthy(other).then(String::new),
    }
}

pub fn artists(items: &mut [SsArtist], sort: ArtistSort, order: SortOrder) {
    let desc = order == SortOrder::Desc;
    match sort {
        ArtistSort::AlbumCount => {
            order_by(items, false, |x| vec![s(Some(&x.name))]);
            order_by(items, desc, |x| vec![n(x.album_count.map(|c| c as i64))]);
        }
        ArtistSort::Favorited => order_by(items, desc, |x| vec![Val::S(starred(x))]),
        ArtistSort::Name => order_by(items, desc, |x| vec![s(Some(&x.name))]),
        ArtistSort::Rating => order_by(items, desc, |x| vec![n(x.user_rating.map(|r| r as i64))]),
        ArtistSort::SongCount | ArtistSort::PlayCount | ArtistSort::Random => {}
    }
}

/// `limit == 0` returns everything from `start`, like the TS `sortAndPaginate`.
pub fn paginate<T>(items: Vec<T>, start: usize, limit: usize) -> Vec<T> {
    let iter = items.into_iter().skip(start);
    if limit == 0 {
        iter.collect()
    } else {
        iter.take(limit).collect()
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
        items.iter().map(|s| s.title.as_str()).collect()
    }

    #[test]
    fn name_sort_is_case_insensitive() {
        let mut items = vec![
            song("b", None, None),
            song("A", None, None),
            song("c", None, None),
        ];
        songs(&mut items, SongSort::Name, SortOrder::Asc);
        assert_eq!(titles(&items), ["A", "b", "c"]);
        songs(&mut items, SongSort::Name, SortOrder::Desc);
        assert_eq!(titles(&items), ["c", "b", "A"]);
    }

    #[test]
    fn missing_values_sort_last_ascending_first_descending() {
        let mut items = vec![
            song("x", None, None),
            song("y", None, Some(3)),
            song("z", None, Some(5)),
        ];
        songs(&mut items, SongSort::Rating, SortOrder::Asc);
        assert_eq!(titles(&items), ["y", "z", "x"]);
        songs(&mut items, SongSort::Rating, SortOrder::Desc);
        assert_eq!(titles(&items), ["x", "z", "y"]);
    }

    #[test]
    fn paginate_slices() {
        assert_eq!(paginate(vec![1, 2, 3, 4], 1, 2), vec![2, 3]);
        assert_eq!(paginate(vec![1, 2, 3, 4], 2, 0), vec![3, 4]);
        assert!(paginate(vec![1, 2], 5, 2).is_empty());
    }
}
