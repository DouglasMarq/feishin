use feishin_core::model::Artist;
use slint::Image;

use super::super::ArtistCard;

#[derive(Debug, Clone, PartialEq)]
pub struct ArtistItem {
    pub id: String,
    pub name: String,
    pub album_count: Option<u32>,
    pub song_count: Option<u32>,
    pub favorite: bool,
    pub cover_art_id: Option<String>,
}

impl From<Artist> for ArtistItem {
    fn from(artist: Artist) -> Self {
        Self {
            id: artist.id,
            name: artist.name,
            album_count: artist.album_count,
            song_count: artist.song_count,
            favorite: artist.favorite,
            cover_art_id: artist.cover_art_id,
        }
    }
}

impl ArtistItem {
    pub fn into_card(self, image: Option<Image>) -> ArtistCard {
        ArtistCard {
            albums: count_label(self.album_count, "album", "albums").into(),
            songs: count_label(self.song_count, "song", "songs").into(),
            id: self.id.into(),
            name: self.name.into(),
            favorite: self.favorite,
            has_image: image.is_some(),
            image: image.unwrap_or_default(),
        }
    }
}

pub fn count_label(count: Option<u32>, singular: &str, plural: &str) -> String {
    match count {
        Some(1) => format!("1 {singular}"),
        Some(count) => format!("{count} {plural}"),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_labels() {
        assert_eq!(count_label(Some(1), "album", "albums"), "1 album");
        assert_eq!(count_label(Some(3), "album", "albums"), "3 albums");
        assert_eq!(count_label(None, "album", "albums"), "");
    }
}
