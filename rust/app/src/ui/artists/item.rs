use feishin_core::model::Artist;
use slint::Image;

use super::super::ArtistCard;
use super::super::genres::album_count_label;

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
            favorite: artist.is_favorite,
            cover_art_id: artist.cover_art_id,
        }
    }
}

impl ArtistItem {
    pub fn into_card(self, image: Option<Image>) -> ArtistCard {
        ArtistCard {
            albums: album_count_label(self.album_count).into(),
            songs: song_count_label(self.song_count).into(),
            id: self.id.into(),
            name: self.name.into(),
            favorite: self.favorite,
            has_image: image.is_some(),
            image: image.unwrap_or_default(),
        }
    }
}

pub fn song_count_label(count: Option<u32>) -> String {
    count
        .map(|count| tr_plural!("{n} song", "{n} songs", count))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_labels() {
        assert_eq!(album_count_label(Some(1)), "1 album");
        assert_eq!(album_count_label(Some(3)), "3 albums");
        assert_eq!(album_count_label(None), "");
        assert_eq!(song_count_label(Some(1)), "1 song");
        assert_eq!(song_count_label(Some(3)), "3 songs");
        assert_eq!(song_count_label(None), "");
    }
}
