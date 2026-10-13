use feishin_youtube::music::{CollectionKind, MusicCollection, MusicSong, ShelfItem};
use slint::Image;

use crate::ui::{DiscoverCard, DiscoverCardKind, DiscoverSong, clamp_i32, format_duration};

pub fn card_of(item: &ShelfItem) -> DiscoverCard {
    match item {
        ShelfItem::Song(song) => song_card(song),
        ShelfItem::Collection(collection) => collection_card(collection),
    }
}

fn song_card(song: &MusicSong) -> DiscoverCard {
    DiscoverCard {
        kind: DiscoverCardKind::Song,
        id: song.video_id.as_str().into(),
        title: song.title.as_str().into(),
        subtitle: song.artist_names().into(),
        cover: Image::default(),
        has_cover: false,
        can_load_cover: song.thumbnail_url.is_some(),
    }
}

fn collection_card(collection: &MusicCollection) -> DiscoverCard {
    DiscoverCard {
        kind: match collection.kind {
            CollectionKind::Album => DiscoverCardKind::Album,
            CollectionKind::Playlist => DiscoverCardKind::Playlist,
        },
        id: collection.browse_id.as_str().into(),
        title: collection.title.as_str().into(),
        subtitle: collection.subtitle.as_str().into(),
        cover: Image::default(),
        has_cover: false,
        can_load_cover: collection.thumbnail_url.is_some(),
    }
}

pub fn thumbnail_of(item: &ShelfItem) -> Option<&str> {
    match item {
        ShelfItem::Song(song) => song.thumbnail_url.as_deref(),
        ShelfItem::Collection(collection) => collection.thumbnail_url.as_deref(),
    }
}

pub fn song_row(position: usize, song: &MusicSong) -> DiscoverSong {
    DiscoverSong {
        id: feishin_core::queue::youtube_song_id(&song.video_id).into(),
        index: clamp_i32(position as u64 + 1),
        title: song.title.as_str().into(),
        artists: song.artist_names().into(),
        album: song.album.as_deref().unwrap_or_default().into(),
        duration: song
            .duration_ms
            .map(format_duration)
            .unwrap_or_default()
            .into(),
        cover: Image::default(),
        has_cover: false,
        can_load_cover: song.thumbnail_url.is_some(),
    }
}

pub fn cover_key(url: &str) -> String {
    format!("youtube-music:{url}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song() -> MusicSong {
        MusicSong {
            video_id: "abc".into(),
            title: "Song".into(),
            artists: vec!["One".into(), "Two".into()],
            album: Some("Album".into()),
            duration_ms: Some(61_000),
            thumbnail_url: Some("https://example.invalid/a.jpg".into()),
        }
    }

    #[test]
    fn songs_become_cards_and_rows() {
        let card = card_of(&ShelfItem::Song(song()));
        assert_eq!(card.kind, DiscoverCardKind::Song);
        assert_eq!(card.subtitle, "One, Two");
        assert!(card.can_load_cover);
        let row = song_row(4, &song());
        assert_eq!(
            (row.index, row.id.as_str(), row.duration.as_str()),
            (5, "yt:abc", "1:01")
        );
    }

    #[test]
    fn collections_keep_their_kind_and_browse_id() {
        let album = ShelfItem::Collection(MusicCollection {
            browse_id: "MPREb_x".into(),
            kind: CollectionKind::Album,
            title: "Record".into(),
            subtitle: "Album • One".into(),
            thumbnail_url: None,
        });
        let card = card_of(&album);
        assert_eq!(
            (card.kind, card.id.as_str()),
            (DiscoverCardKind::Album, "MPREb_x")
        );
        assert!(!card.can_load_cover);
        assert_eq!(thumbnail_of(&album), None);
    }
}
