use feishin_youtube::music::{MusicShelf, MusicSong, ShelfItem};
use slint::{Image, Model};

use super::cover_slots::MAX_SHOWN_COVERS;
use super::model::DiscoverModel;
use crate::discover::FeedRequest;

const MANY: usize = MAX_SHOWN_COVERS + 12;

fn song(number: usize) -> MusicSong {
    MusicSong {
        video_id: format!("video{number:06}"),
        title: format!("Song {number}"),
        artists: vec!["Artist".into()],
        album: None,
        duration_ms: None,
        thumbnail_url: Some(format!("https://example.invalid/{number}.jpg")),
    }
}

fn wide_shelf() -> MusicShelf {
    MusicShelf {
        title: "Wide".into(),
        items: (0..MANY)
            .map(|number| ShelfItem::Song(song(number)))
            .collect(),
    }
}

fn load_card(model: &DiscoverModel, card: usize) {
    let (request, card_id) = model.request_card_cover(0, card).unwrap();
    model.set_card_cover(request.generation, 0, &card_id, Some(Image::default()));
}

fn cards_with_covers(model: &DiscoverModel) -> Vec<bool> {
    model.cards(0).iter().map(|card| card.has_cover).collect()
}

#[test]
fn every_card_on_screen_keeps_its_cover_on_a_large_window() {
    let model = DiscoverModel::default();
    model.show_shelves(Some(&FeedRequest::NewReleases), &[wide_shelf()]);
    for card in 0..MANY {
        model.set_card_visible(0, card, true);
        load_card(&model, card);
    }
    assert!(cards_with_covers(&model).iter().all(|has_cover| *has_cover));
    for card in 0..20 {
        model.set_card_visible(0, card, false);
    }
    let kept = cards_with_covers(&model);
    assert_eq!(
        kept.iter().filter(|has_cover| **has_cover).count(),
        MAX_SHOWN_COVERS
    );
    assert!(kept.iter().skip(20).all(|has_cover| *has_cover));
}

#[test]
fn every_song_row_on_screen_keeps_its_cover_on_a_tall_window() {
    let model = DiscoverModel::default();
    model.page.borrow_mut().songs = (0..MANY).map(song).collect();
    model.show_page_songs();
    model.show_song_rows(0..MANY);
    for index in 0..MANY {
        let request = model.request_song_cover(index).unwrap();
        model.set_song_cover(request.generation, index, Some(Image::default()));
    }
    let covered = |model: &DiscoverModel| model.songs.iter().filter(|row| row.has_cover).count();
    assert_eq!(covered(&model), MANY);
    model.show_song_rows(MANY - 10..MANY);
    assert_eq!(covered(&model), MAX_SHOWN_COVERS);
    assert!(model.songs.row_data(MANY - 1).unwrap().has_cover);
}
