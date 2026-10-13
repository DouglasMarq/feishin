use feishin_core::model::{Album, ArtistRef};
use slint::SharedString;

pub fn credited_artists(album: &Album) -> &[ArtistRef] {
    Some(album.album_artists.as_slice())
        .filter(|album_artists| !album_artists.is_empty())
        .unwrap_or(&album.artists)
}

pub fn credited_artist_names(album: &Album) -> String {
    credited_artists(album)
        .iter()
        .map(|artist| artist.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn first_credited_artist_id(album: &Album) -> &str {
    credited_artists(album)
        .first()
        .map(|artist| artist.id.as_str())
        .unwrap_or_default()
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CardData {
    pub id: SharedString,
    pub name: SharedString,
    pub artists: SharedString,
    pub artist_id: SharedString,
    pub year: SharedString,
    pub cover_art_id: Option<String>,
}

impl CardData {
    pub fn from_album(album: &Album) -> Self {
        Self {
            id: album.id.as_str().into(),
            name: album.name.as_str().into(),
            artists: credited_artist_names(album).into(),
            artist_id: first_credited_artist_id(album).into(),
            year: album
                .year
                .map(|year| year.to_string())
                .unwrap_or_default()
                .into(),
            cover_art_id: album.cover_art_id.clone(),
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn album(index: usize) -> Album {
        Album {
            id: format!("album-{index}"),
            server_id: "demo".into(),
            name: format!("Album {index}"),
            album_artists: Vec::new(),
            artists: vec![ArtistRef {
                id: "artist-1".into(),
                name: "Ada Vale".into(),
            }],
            year: Some(1999),
            genres: Vec::new(),
            song_count: 10,
            duration_ms: 0,
            cover_art_id: Some(format!("al-{index}")),
            play_count: 0,
            user_rating: None,
            favorite: false,
            created_at: None,
            last_played_at: None,
        }
    }

    #[test]
    fn cards_prefer_album_artists_and_fall_back_to_artists() {
        let card = CardData::from_album(&album(3));
        assert_eq!(card.artists, "Ada Vale");
        assert_eq!(card.artist_id, "artist-1");
        assert_eq!(card.year, "1999");
        assert_eq!(card.cover_art_id.as_deref(), Some("al-3"));
    }
}
