use feishin_core::model::{AlbumListQuery, ArtistListQuery, SongListQuery};

use crate::features::Features;
use crate::query::sort::{album_sort, artist_sort, order, song_sort};

pub(crate) type QueryParams = Vec<(&'static str, String)>;

const UNBOUNDED_END: i64 = -1;

pub(crate) fn end_index_or_all(start: usize, limit: usize) -> i64 {
    let span = match limit {
        0 => UNBOUNDED_END,
        limit => limit as i64,
    };
    start as i64 + span
}

pub(crate) fn end_index(start: usize, limit: usize) -> i64 {
    (start + limit) as i64
}

pub(crate) fn song_list(
    query: &SongListQuery,
    start: usize,
    end: i64,
    album_ids: &[String],
    features: &Features,
) -> QueryParams {
    let mut params = sorted_page(start, end, Some(song_sort(query.sort)), order(query.order));
    push_all(&mut params, "album_id", album_ids);
    push_all(&mut params, "genre_id", &query.genre_ids);
    push_all(&mut params, song_artist_key(features), &query.artist_ids);
    push_optional(&mut params, "library_id", query.music_folder_id.clone());
    push_optional(&mut params, "starred", favorite_filter(query.favorite));
    push_optional(&mut params, "title", search_filter(query.search.as_ref()));
    push_optional(
        &mut params,
        "year",
        year_filter(query.year_from, query.year_to),
    );
    push_missing(&mut params, features);
    params
}

pub(crate) fn album_list(
    query: &AlbumListQuery,
    start: usize,
    end: i64,
    features: &Features,
) -> QueryParams {
    let mut params = sorted_page(start, end, Some(album_sort(query.sort)), order(query.order));
    push_all(
        &mut params,
        "artist_id",
        repeatable(&query.artist_ids, features),
    );
    push_all(
        &mut params,
        "genre_id",
        repeatable(&query.genre_ids, features),
    );
    push_optional(&mut params, "library_id", query.music_folder_id.clone());
    push_optional(&mut params, "name", search_filter(query.search.as_ref()));
    push_optional(&mut params, "starred", favorite_filter(query.favorite));
    push_optional(
        &mut params,
        "year",
        year_filter(query.year_from, query.year_to),
    );
    push_missing(&mut params, features);
    params
}

pub(crate) fn artist_list(
    query: &ArtistListQuery,
    start: usize,
    end: i64,
    features: &Features,
) -> QueryParams {
    let mut params = sorted_page(start, end, artist_sort(query.sort), order(query.order));
    push_optional(&mut params, "library_id", query.music_folder_id.clone());
    push_optional(&mut params, "name", search_filter(query.search.as_ref()));
    push_optional(&mut params, "starred", favorite_filter(query.favorite));
    if features.has_big_feature_release() {
        params.push(("role", "albumartist".into()));
    }
    push_missing(&mut params, features);
    params
}

pub(crate) fn album_songs(album_id: &str, features: &Features) -> QueryParams {
    let mut params = sorted_page(0, 0, Some("album"), "ASC");
    params.push(("album_id", album_id.to_owned()));
    push_missing(&mut params, features);
    params
}

pub(crate) fn playlist_tracks(features: &Features) -> QueryParams {
    let mut params = sorted_page(0, UNBOUNDED_END, Some("id"), "ASC");
    push_missing(&mut params, features);
    params
}

fn sorted_page(
    start: usize,
    end: i64,
    sort: Option<&'static str>,
    order: &'static str,
) -> QueryParams {
    let mut params = vec![("_start", start.to_string()), ("_end", end.to_string())];
    push_optional(&mut params, "_sort", sort.map(str::to_owned));
    params.push(("_order", order.to_owned()));
    params
}

fn push_all(params: &mut QueryParams, key: &'static str, values: &[String]) {
    params.extend(values.iter().map(|value| (key, value.clone())));
}

fn push_optional(params: &mut QueryParams, key: &'static str, value: Option<String>) {
    params.extend(value.map(|value| (key, value)));
}

fn push_missing(params: &mut QueryParams, features: &Features) {
    if features.has_big_feature_release() {
        params.push(("missing", "false".into()));
    }
}

fn song_artist_key(features: &Features) -> &'static str {
    if features.supports_track_artist_filter() {
        return "artists_id";
    }
    "album_artist_id"
}

fn repeatable<'values>(values: &'values [String], features: &Features) -> &'values [String] {
    if features.has_big_feature_release() {
        return values;
    }
    values.get(..1).unwrap_or(values)
}

fn favorite_filter(favorite: Option<bool>) -> Option<String> {
    favorite.map(|favorite| favorite.to_string())
}

fn search_filter(search: Option<&String>) -> Option<String> {
    search.filter(|search| !search.is_empty()).cloned()
}

fn year_filter(year_from: Option<i32>, year_to: Option<i32>) -> Option<String> {
    year_to.or(year_from).map(|year| year.to_string())
}

#[cfg(test)]
mod tests {
    use feishin_core::model::{SongSort, SortOrder};

    use super::*;
    use crate::features::ServerVersion;

    fn features(version: ServerVersion) -> Features {
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
        let params = song_list(
            &query,
            10,
            end_index_or_all(10, 5),
            &[],
            &features(ServerVersion::new(0, 58, 0)),
        );
        assert!(params.contains(&("_start", "10".into())));
        assert!(params.contains(&("_end", "15".into())));
        assert!(params.contains(&("_sort", "createdAt".into())));
        assert!(params.contains(&("_order", "DESC".into())));
        assert!(params.contains(&("artists_id", "a2".into())));
        assert!(params.contains(&("year", "1999".into())));
        assert!(params.contains(&("missing", "false".into())));
        assert!(!params.iter().any(|(key, _)| *key == "title"));
    }

    #[test]
    fn album_list_params_old_server() {
        let query = AlbumListQuery {
            genre_ids: vec!["g1".into(), "g2".into()],
            ..AlbumListQuery::default()
        };
        let params = album_list(
            &query,
            0,
            end_index(0, 0),
            &features(ServerVersion::new(0, 54, 0)),
        );
        let genres: Vec<_> = params
            .iter()
            .filter(|(key, _)| *key == "genre_id")
            .collect();
        assert_eq!(genres.len(), 1);
        assert!(!params.iter().any(|(key, _)| *key == "missing"));
        assert!(params.contains(&("_end", "0".into())));
    }

    #[test]
    fn end_helpers() {
        assert_eq!(end_index_or_all(0, 0), -1);
        assert_eq!(end_index_or_all(20, 50), 70);
        assert_eq!(end_index(5, 0), 5);
    }
}
