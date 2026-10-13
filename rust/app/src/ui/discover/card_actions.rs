use feishin_youtube::music::ShelfItem;

use super::browser::open_in_browser;
use super::deps::DiscoverDeps;
use super::model::with_discover;
use super::playing::{Placement, play_collection, play_songs};
use crate::ui::AppWindow;

fn item_at(window: &AppWindow, shelf: usize, card: usize) -> Option<ShelfItem> {
    with_discover(window, |model| model.item(shelf, card)).flatten()
}

pub fn play_from_card(window: &AppWindow, deps: &DiscoverDeps, shelf: usize, card: usize) {
    let shelf_songs = with_discover(window, |model| model.shelf_songs_from(shelf, card)).flatten();
    if let Some((songs, start)) = shelf_songs {
        play_songs(window, deps, songs, Placement::PlayFrom(start));
    }
}

pub fn place_card(
    window: &AppWindow,
    deps: &DiscoverDeps,
    (shelf, card): (usize, usize),
    placement: Placement,
) {
    match item_at(window, shelf, card) {
        Some(ShelfItem::Song(song)) => play_songs(window, deps, vec![song], placement),
        Some(ShelfItem::Collection(collection)) => {
            play_collection(window, deps, collection.browse_id, placement);
        }
        None => {}
    }
}

pub fn open_card_in_browser(window: &AppWindow, deps: &DiscoverDeps, shelf: usize, card: usize) {
    let url = match item_at(window, shelf, card) {
        Some(ShelfItem::Song(song)) => song.web_url(),
        Some(ShelfItem::Collection(collection)) => collection.web_url(),
        None => return,
    };
    open_in_browser(window, deps, url);
}
