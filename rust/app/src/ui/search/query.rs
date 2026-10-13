use feishin_core::model::{AlbumListQuery, ArtistListQuery, SongListQuery};

pub const SEARCH_LIMIT: usize = 20;

pub fn normalize(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

pub fn song_query(search: &str) -> SongListQuery {
    SongListQuery {
        search: Some(search.to_owned()),
        ..SongListQuery::default()
    }
}

pub fn album_query(search: &str) -> AlbumListQuery {
    AlbumListQuery {
        search: Some(search.to_owned()),
        ..AlbumListQuery::default()
    }
}

pub fn artist_query(search: &str) -> ArtistListQuery {
    ArtistListQuery {
        search: Some(search.to_owned()),
        ..ArtistListQuery::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_queries_are_ignored() {
        assert_eq!(normalize("   "), None);
        assert_eq!(normalize("  river "), Some("river".to_owned()));
        assert_eq!(song_query("river").search.as_deref(), Some("river"));
        assert_eq!(album_query("river").search.as_deref(), Some("river"));
        assert_eq!(artist_query("river").search.as_deref(), Some("river"));
    }
}
