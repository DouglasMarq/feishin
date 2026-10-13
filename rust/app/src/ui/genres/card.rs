use feishin_api::MusicServer;
use feishin_core::model::{
    Album, AlbumListQuery, Artist, ArtistListQuery, ArtistRef, ItemKind, Paging,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardData {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub cover_id: Option<String>,
    pub favorite: bool,
}

pub struct CardPage {
    pub cards: Vec<CardData>,
    pub total: Option<usize>,
}

fn credited_artists(album: &Album) -> &[ArtistRef] {
    Some(album.album_artists.as_slice())
        .filter(|album_artists| !album_artists.is_empty())
        .unwrap_or(&album.artists)
}

pub fn album_artist_names(album: &Album) -> String {
    credited_artists(album)
        .iter()
        .map(|artist| artist.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn album_count_label(album_count: Option<u32>) -> String {
    match album_count {
        Some(1) => "1 album".to_owned(),
        Some(count) => format!("{count} albums"),
        None => String::new(),
    }
}

pub fn album_card(album: &Album) -> CardData {
    CardData {
        id: album.id.clone(),
        title: album.name.clone(),
        subtitle: album_artist_names(album),
        cover_id: album.cover_art_id.clone(),
        favorite: album.favorite,
    }
}

pub fn artist_card(artist: &Artist) -> CardData {
    CardData {
        id: artist.id.clone(),
        title: artist.name.clone(),
        subtitle: album_count_label(artist.album_count),
        cover_id: artist.cover_art_id.clone(),
        favorite: artist.favorite,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardSource {
    Albums(AlbumListQuery),
    Artists(ArtistListQuery),
}

impl CardSource {
    pub fn kind(&self) -> ItemKind {
        match self {
            Self::Albums(_) => ItemKind::Album,
            Self::Artists(_) => ItemKind::Artist,
        }
    }

    pub async fn fetch(
        &self,
        server: &dyn MusicServer,
        paging: Paging,
    ) -> feishin_api::Result<CardPage> {
        Ok(match self {
            Self::Albums(query) => {
                let page = server.album_list(query, paging).await?;
                CardPage {
                    cards: page.items.iter().map(album_card).collect(),
                    total: page.total_count,
                }
            }
            Self::Artists(query) => {
                let page = server.artist_list(query, paging).await?;
                CardPage {
                    cards: page.items.iter().map(artist_card).collect(),
                    total: page.total_count,
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn album_by_ada() -> Album {
        Album {
            id: "al".into(),
            server_id: "s".into(),
            name: "Name".into(),
            album_artists: vec![ArtistRef {
                id: "ar".into(),
                name: "Ada".into(),
            }],
            artists: Vec::new(),
            year: None,
            genres: Vec::new(),
            song_count: 1,
            duration_ms: 0,
            cover_art_id: Some("c".into()),
            play_count: 0,
            user_rating: None,
            favorite: true,
            created_at: None,
            last_played_at: None,
        }
    }

    fn artist_with_three_albums() -> Artist {
        Artist {
            id: "ar".into(),
            server_id: "s".into(),
            name: "Ada".into(),
            album_count: Some(3),
            song_count: None,
            cover_art_id: None,
            favorite: false,
            user_rating: None,
            biography: None,
        }
    }

    #[test]
    fn cards_map_albums_and_artists() {
        let card = album_card(&album_by_ada());
        assert_eq!(
            (
                card.subtitle.as_str(),
                card.cover_id.as_deref(),
                card.favorite
            ),
            ("Ada", Some("c"), true)
        );
        assert_eq!(
            artist_card(&artist_with_three_albums()).subtitle,
            "3 albums"
        );
    }
}
