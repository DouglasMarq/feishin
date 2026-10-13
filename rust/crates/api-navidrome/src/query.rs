//! Native API query parameters, mirroring the sort maps in `domain-types.ts` and the filters in
//! `navidrome-controller.ts`.

use feishin_core::model::{
    AlbumListQuery, AlbumSort, ArtistListQuery, ArtistSort, SongListQuery, SongSort, SortOrder,
};

use crate::features::Features;

pub(crate) type Params = Vec<(&'static str, String)>;

pub(crate) fn order(order: SortOrder) -> &'static str {
    match order {
        SortOrder::Asc => "ASC",
        SortOrder::Desc => "DESC",
    }
}

pub(crate) fn song_sort(sort: SongSort) -> &'static str {
    match sort {
        SongSort::Name => "title",
        SongSort::Album => "album",
        SongSort::AlbumArtist => "order_album_artist_name",
        SongSort::Artist => "artist",
        SongSort::Duration => "duration",
        SongSort::Year => "year",
        SongSort::PlayCount => "playCount",
        SongSort::Rating => "rating",
        SongSort::Favorited => "starred_at",
        SongSort::RecentlyAdded => "createdAt",
        SongSort::RecentlyPlayed => "playDate",
        SongSort::Random => "random",
    }
}

pub(crate) fn album_sort(sort: AlbumSort) -> &'static str {
    match sort {
        AlbumSort::Name => "name",
        AlbumSort::AlbumArtist => "album_artist",
        AlbumSort::Year => "max_year",
        AlbumSort::PlayCount => "play_count",
        AlbumSort::Rating => "rating",
        AlbumSort::Favorited => "starred_at",
        AlbumSort::RecentlyAdded => "recently_added",
        AlbumSort::RecentlyPlayed => "play_date",
        AlbumSort::SongCount => "songCount",
        AlbumSort::Duration => "duration",
        AlbumSort::Random => "random",
    }
}

pub(crate) fn artist_sort(sort: ArtistSort) -> Option<&'static str> {
    match sort {
        ArtistSort::Name => Some("name"),
        ArtistSort::AlbumCount => Some("albumCount"),
        ArtistSort::SongCount => Some("songCount"),
        ArtistSort::PlayCount => Some("playCount"),
        ArtistSort::Rating => Some("rating"),
        ArtistSort::Favorited => Some("starred_at"),
        ArtistSort::Random => None,
    }
}

fn push_all(params: &mut Params, key: &'static str, values: &[String]) {
    params.extend(values.iter().map(|v| (key, v.clone())));
}

fn push_missing(params: &mut Params, features: &Features) {
    if features.bfr() {
        params.push(("missing", "false".into()));
    }
}

fn push_paging(params: &mut Params, start: usize, end: i64) {
    params.push(("_start", start.to_string()));
    params.push(("_end", end.to_string()));
}

/// `_end` for lists where an unset limit means "everything" (`startIndex + (limit || -1)`).
pub(crate) fn end_or_all(start: usize, limit: usize) -> i64 {
    let limit = if limit == 0 { -1 } else { limit as i64 };
    start as i64 + limit
}

/// `_end` for lists using `startIndex + (limit || 0)`.
pub(crate) fn end(start: usize, limit: usize) -> i64 {
    (start + limit) as i64
}

pub(crate) fn song_list(
    query: &SongListQuery,
    start: usize,
    end: i64,
    album_ids: &[String],
    features: &Features,
) -> Params {
    let mut p = Params::new();
    push_paging(&mut p, start, end);
    p.push(("_sort", song_sort(query.sort).into()));
    p.push(("_order", order(query.order).into()));
    push_all(&mut p, "album_id", album_ids);
    push_all(&mut p, "genre_id", &query.genre_ids);
    let artist_key = if features.track_artist_filter() {
        "artists_id"
    } else {
        "album_artist_id"
    };
    push_all(&mut p, artist_key, &query.artist_ids);
    if let Some(folder) = &query.music_folder_id {
        p.push(("library_id", folder.clone()));
    }
    if let Some(favorite) = query.favorite {
        p.push(("starred", favorite.to_string()));
    }
    if let Some(search) = query.search.as_ref().filter(|s| !s.is_empty()) {
        p.push(("title", search.clone()));
    }
    if let Some(year) = query.year_to.or(query.year_from) {
        p.push(("year", year.to_string()));
    }
    push_missing(&mut p, features);
    p
}

pub(crate) fn album_list(
    query: &AlbumListQuery,
    start: usize,
    end: i64,
    features: &Features,
) -> Params {
    let mut p = Params::new();
    push_paging(&mut p, start, end);
    p.push(("_sort", album_sort(query.sort).into()));
    p.push(("_order", order(query.order).into()));
    let repeatable = |values: &[String]| -> Vec<String> {
        if features.bfr() {
            values.to_vec()
        } else {
            values.iter().take(1).cloned().collect()
        }
    };
    push_all(&mut p, "artist_id", &repeatable(&query.artist_ids));
    push_all(&mut p, "genre_id", &repeatable(&query.genre_ids));
    if let Some(folder) = &query.music_folder_id {
        p.push(("library_id", folder.clone()));
    }
    if let Some(search) = query.search.as_ref().filter(|s| !s.is_empty()) {
        p.push(("name", search.clone()));
    }
    if let Some(favorite) = query.favorite {
        p.push(("starred", favorite.to_string()));
    }
    if let Some(year) = query.year_to.or(query.year_from) {
        p.push(("year", year.to_string()));
    }
    push_missing(&mut p, features);
    p
}

pub(crate) fn artist_list(
    query: &ArtistListQuery,
    start: usize,
    end: i64,
    features: &Features,
) -> Params {
    let mut p = Params::new();
    push_paging(&mut p, start, end);
    if let Some(sort) = artist_sort(query.sort) {
        p.push(("_sort", sort.into()));
    }
    p.push(("_order", order(query.order).into()));
    if let Some(folder) = &query.music_folder_id {
        p.push(("library_id", folder.clone()));
    }
    if let Some(search) = query.search.as_ref().filter(|s| !s.is_empty()) {
        p.push(("name", search.clone()));
    }
    if let Some(favorite) = query.favorite {
        p.push(("starred", favorite.to_string()));
    }
    if features.bfr() {
        p.push(("role", "albumartist".into()));
    }
    push_missing(&mut p, features);
    p
}

/// Every track of an album, in album order (as `getAlbumDetail` does).
pub(crate) fn album_songs(album_id: &str, features: &Features) -> Params {
    let mut p = Params::new();
    push_paging(&mut p, 0, 0);
    p.push(("_sort", "album".into()));
    p.push(("_order", "ASC".into()));
    p.push(("album_id", album_id.to_owned()));
    push_missing(&mut p, features);
    p
}

/// Every track of a playlist in playlist order.
pub(crate) fn playlist_tracks(features: &Features) -> Params {
    let mut p = Params::new();
    push_paging(&mut p, 0, -1);
    p.push(("_sort", "id".into()));
    p.push(("_order", "ASC".into()));
    push_missing(&mut p, features);
    p
}

/// Subsonic `getAlbumList2` type for a sort, when one exists.
pub(crate) fn subsonic_album_type(sort: AlbumSort) -> Option<&'static str> {
    match sort {
        AlbumSort::Name => Some("alphabeticalByName"),
        AlbumSort::AlbumArtist => Some("alphabeticalByArtist"),
        AlbumSort::Favorited => Some("starred"),
        AlbumSort::PlayCount => Some("frequent"),
        AlbumSort::Random => Some("random"),
        AlbumSort::RecentlyAdded => Some("newest"),
        AlbumSort::RecentlyPlayed => Some("recent"),
        AlbumSort::Year => Some("byYear"),
        AlbumSort::Rating | AlbumSort::SongCount | AlbumSort::Duration => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn features(version: (u32, u32, u32)) -> Features {
        Features {
            version: Some(version),
            ..Features::default()
        }
    }

    #[test]
    fn song_list_params_new_server() {
        let query = SongListQuery {
            sort: SongSort::RecentlyAdded,
            order: SortOrder::Desc,
            artist_ids: vec!["a1".into(), "a2".into()],
            year_from: Some(1990),
            year_to: Some(1999),
            search: Some(String::new()),
            ..SongListQuery::default()
        };
        let p = song_list(&query, 10, end_or_all(10, 5), &[], &features((0, 58, 0)));
        assert!(p.contains(&("_start", "10".into())));
        assert!(p.contains(&("_end", "15".into())));
        assert!(p.contains(&("_sort", "createdAt".into())));
        assert!(p.contains(&("_order", "DESC".into())));
        assert!(p.contains(&("artists_id", "a2".into())));
        assert!(p.contains(&("year", "1999".into())));
        assert!(p.contains(&("missing", "false".into())));
        assert!(!p.iter().any(|(k, _)| *k == "title"));
    }

    #[test]
    fn album_list_params_old_server() {
        let query = AlbumListQuery {
            genre_ids: vec!["g1".into(), "g2".into()],
            ..AlbumListQuery::default()
        };
        let p = album_list(&query, 0, end(0, 0), &features((0, 54, 0)));
        let genres: Vec<_> = p.iter().filter(|(k, _)| *k == "genre_id").collect();
        assert_eq!(genres.len(), 1);
        assert!(!p.iter().any(|(k, _)| *k == "missing"));
        assert!(p.contains(&("_end", "0".into())));
    }

    #[test]
    fn end_helpers() {
        assert_eq!(end_or_all(0, 0), -1);
        assert_eq!(end_or_all(20, 50), 70);
        assert_eq!(end(5, 0), 5);
    }
}
