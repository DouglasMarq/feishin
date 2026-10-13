use feishin_core::model::{Album, Artist};

use crate::ui::genres::{album_artist_names, album_count_label};
use crate::ui::{SearchCard, SearchKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CardKind {
    Albums,
    Artists,
}

impl From<SearchKind> for CardKind {
    fn from(kind: SearchKind) -> Self {
        match kind {
            SearchKind::Albums => Self::Albums,
            SearchKind::Songs | SearchKind::Artists => Self::Artists,
        }
    }
}

pub fn album_card(album: &Album) -> SearchCard {
    SearchCard {
        id: album.id.as_str().into(),
        title: album.name.as_str().into(),
        subtitle: album_artist_names(album).into(),
        ..SearchCard::default()
    }
}

pub fn artist_card(artist: &Artist) -> SearchCard {
    SearchCard {
        id: artist.id.as_str().into(),
        title: artist.name.as_str().into(),
        subtitle: album_count_label(artist.album_count).into(),
        ..SearchCard::default()
    }
}

#[cfg(test)]
mod tests {
    use feishin_api::MusicServer;
    use feishin_core::model::SearchResults;

    use super::*;
    use crate::demo::FakeServer;
    use crate::ui::search::query::SEARCH_LIMIT;

    #[tokio::test]
    async fn demo_results_map_to_cards() {
        let server = FakeServer::new(1000);
        let found = server.search("ada", SEARCH_LIMIT).await.ok();
        let found = found.unwrap_or(SearchResults {
            songs: Vec::new(),
            albums: Vec::new(),
            artists: Vec::new(),
        });
        let album = found.albums.first().map(album_card).unwrap_or_default();
        assert!(album.id.starts_with("album-"));
        assert!(album.subtitle.contains("Ada"));
        let artist = found.artists.first().map(artist_card).unwrap_or_default();
        assert!(artist.title.starts_with("Ada"));
    }
}
