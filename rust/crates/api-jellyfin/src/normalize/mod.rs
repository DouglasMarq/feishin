mod album;
mod artist;
mod library;
mod lyrics;
mod playlist;
mod song;

use feishin_core::model::ArtistRef;

use crate::dto;

pub use album::album;
pub use artist::artist;
pub use library::{genre, music_folder};
pub use lyrics::lyrics;
pub use playlist::playlist;
pub use song::song;

pub const TICKS_PER_MS: u64 = 10_000;

fn ticks_to_ms(ticks: Option<u64>) -> u64 {
    ticks.unwrap_or(0) / TICKS_PER_MS
}

fn artist_refs(items: &[dto::NamedItem]) -> Vec<ArtistRef> {
    items
        .iter()
        .map(|item| ArtistRef {
            id: item.id.clone(),
            name: item.name.clone(),
        })
        .collect()
}

fn credited_artists(
    artist_items: &[dto::NamedItem],
    album_artists: &[dto::NamedItem],
) -> Vec<ArtistRef> {
    match artist_items {
        [] => artist_refs(album_artists),
        track_artists => artist_refs(track_artists),
    }
}

fn names(items: &[dto::NamedItem]) -> Vec<String> {
    items.iter().map(|item| item.name.clone()).collect()
}

fn primary_image(id: &str, tags: Option<&dto::ImageTags>) -> Option<String> {
    tags.and_then(|tags| tags.primary.as_ref())
        .map(|_primary_tag| id.to_owned())
}

fn non_empty(text: Option<String>) -> Option<String> {
    text.filter(|content| !content.is_empty())
}

fn release_year(premiere_date: Option<&str>, production_year: Option<i32>) -> Option<i32> {
    premiere_date
        .and_then(|date| date.get(..4))
        .and_then(|year| year.parse::<i32>().ok())
        .filter(|year| *year > 0)
        .or(production_year.filter(|year| *year > 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn year_prefers_premiere_date() {
        assert_eq!(
            release_year(Some("2019-05-01T00:00:00Z"), Some(2001)),
            Some(2019)
        );
        assert_eq!(release_year(None, Some(2001)), Some(2001));
        assert_eq!(release_year(Some("bad"), Some(0)), None);
    }
}
