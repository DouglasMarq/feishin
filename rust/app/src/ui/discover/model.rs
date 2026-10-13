use std::cell::{Cell, RefCell};
use std::rc::Rc;

use feishin_youtube::music::{MusicShelf, MusicSong, ShelfItem};
use slint::{ComponentHandle, Image, Model, ModelNotify, ModelRc, ModelTracker, VecModel};

use super::cards::{card_of, song_row, thumbnail_of};
use super::cover_slots::CoverSlots;
use super::page::Page;
use super::sections::Sections;
use crate::discover::FeedRequest;
use crate::ui::{AppWindow, DiscoverCard, DiscoverShelf, DiscoverSong, DiscoverState};

type CardKey = (usize, String);

pub struct CoverRequest {
    pub generation: u64,
    pub url: String,
}

struct ShelfSlot {
    title: String,
    items: Vec<ShelfItem>,
    cards: Rc<VecModel<DiscoverCard>>,
}

#[derive(Default)]
pub struct DiscoverModel {
    slots: RefCell<Vec<ShelfSlot>>,
    notify: ModelNotify,
    shown_request: RefCell<Option<FeedRequest>>,
    shelf_generation: Cell<u64>,
    card_covers: RefCell<CoverSlots<CardKey>>,
    pub sections: RefCell<Sections>,
    pub page: RefCell<Page>,
    pub songs: Rc<VecModel<DiscoverSong>>,
    song_covers: RefCell<CoverSlots<usize>>,
    song_generation: Cell<u64>,
}

impl DiscoverModel {
    pub fn show_shelves(&self, request: Option<&FeedRequest>, shelves: &[MusicShelf]) {
        if self.shown_request.borrow().as_ref() == request {
            return;
        }
        *self.shown_request.borrow_mut() = request.cloned();
        self.shelf_generation.set(self.shelf_generation.get() + 1);
        self.card_covers.borrow_mut().clear();
        *self.slots.borrow_mut() = shelves.iter().map(slot_of).collect();
        self.notify.reset();
    }

    pub fn forget_shown_shelves(&self) {
        self.show_shelves(None, &[]);
    }

    pub fn item(&self, shelf: usize, card: usize) -> Option<ShelfItem> {
        self.slots.borrow().get(shelf)?.items.get(card).cloned()
    }

    pub fn shelf_songs_from(&self, shelf: usize, card: usize) -> Option<(Vec<MusicSong>, usize)> {
        let slots = self.slots.borrow();
        let items = &slots.get(shelf)?.items;
        let start = items
            .iter()
            .take(card)
            .filter(|item| matches!(item, ShelfItem::Song(_)))
            .count();
        let songs = items
            .iter()
            .filter_map(|item| match item {
                ShelfItem::Song(song) => Some(song.clone()),
                ShelfItem::Collection(_) => None,
            })
            .collect();
        matches!(items.get(card), Some(ShelfItem::Song(_))).then_some((songs, start))
    }

    pub fn request_card_cover(&self, shelf: usize, card: usize) -> Option<(CoverRequest, String)> {
        let slots = self.slots.borrow();
        let slot = slots.get(shelf)?;
        let shown = slot.cards.row_data(card)?;
        let url = slot.items.get(card).and_then(thumbnail_of)?.to_owned();
        if shown.has_cover {
            return None;
        }
        let card_id = shown.id.to_string();
        let claimed = self
            .card_covers
            .borrow_mut()
            .claim((shelf, card_id.clone()));
        let request = CoverRequest {
            generation: self.shelf_generation.get(),
            url,
        };
        claimed.then_some((request, card_id))
    }

    pub fn set_card_cover(
        &self,
        generation: u64,
        shelf: usize,
        card_id: &str,
        cover: Option<Image>,
    ) {
        let key = (shelf, card_id.to_owned());
        if generation != self.shelf_generation.get() || !self.card_covers.borrow_mut().finish(&key)
        {
            return;
        }
        let Some(cover) = cover else {
            return;
        };
        if self.update_card(shelf, card_id, Some(cover)) {
            let evicted = self.card_covers.borrow_mut().show(key);
            for (evicted_shelf, evicted_id) in evicted {
                self.update_card(evicted_shelf, &evicted_id, None);
            }
        }
    }

    fn update_card(&self, shelf: usize, card_id: &str, cover: Option<Image>) -> bool {
        let slots = self.slots.borrow();
        let Some(cards) = slots.get(shelf).map(|slot| &slot.cards) else {
            return false;
        };
        let Some(index) = cards.iter().position(|card| card.id == card_id) else {
            return false;
        };
        if let Some(mut card) = cards.row_data(index) {
            card.has_cover = cover.is_some();
            card.cover = cover.unwrap_or_default();
            cards.set_row_data(index, card);
        }
        true
    }

    pub fn show_page_songs(&self) {
        self.song_generation.set(self.song_generation.get() + 1);
        self.song_covers.borrow_mut().clear();
        let rows: Vec<DiscoverSong> = self
            .page
            .borrow()
            .songs
            .iter()
            .enumerate()
            .map(|(position, song)| song_row(position, song))
            .collect();
        self.songs.set_vec(rows);
    }

    pub fn page_song(&self, index: usize) -> Option<MusicSong> {
        self.page.borrow().songs.get(index).cloned()
    }

    pub fn request_song_cover(&self, index: usize) -> Option<CoverRequest> {
        let row = self.songs.row_data(index)?;
        let url = self.page.borrow().songs.get(index)?.thumbnail_url.clone()?;
        if row.has_cover || !self.song_covers.borrow_mut().claim(index) {
            return None;
        }
        Some(CoverRequest {
            generation: self.song_generation.get(),
            url,
        })
    }

    pub fn set_song_cover(&self, generation: u64, index: usize, cover: Option<Image>) {
        if generation != self.song_generation.get() || !self.song_covers.borrow_mut().finish(&index)
        {
            return;
        }
        let Some(cover) = cover else {
            return;
        };
        self.update_song(index, Some(cover));
        let evicted = self.song_covers.borrow_mut().show(index);
        for evicted_index in evicted {
            self.update_song(evicted_index, None);
        }
    }

    fn update_song(&self, index: usize, cover: Option<Image>) {
        if let Some(mut row) = self.songs.row_data(index) {
            row.has_cover = cover.is_some();
            row.cover = cover.unwrap_or_default();
            self.songs.set_row_data(index, row);
        }
    }

    #[cfg(test)]
    pub fn cards(&self, shelf: usize) -> Vec<DiscoverCard> {
        self.slots
            .borrow()
            .get(shelf)
            .map(|slot| slot.cards.iter().collect())
            .unwrap_or_default()
    }
}

fn slot_of(shelf: &MusicShelf) -> ShelfSlot {
    ShelfSlot {
        title: shelf.title.clone(),
        items: shelf.items.clone(),
        cards: Rc::new(VecModel::from(
            shelf.items.iter().map(card_of).collect::<Vec<_>>(),
        )),
    }
}

impl Model for DiscoverModel {
    type Data = DiscoverShelf;

    fn row_count(&self) -> usize {
        self.slots.borrow().len()
    }

    fn row_data(&self, row: usize) -> Option<DiscoverShelf> {
        let slots = self.slots.borrow();
        let slot = slots.get(row)?;
        Some(DiscoverShelf {
            title: slot.title.as_str().into(),
            cards: ModelRc::from(Rc::clone(&slot.cards)),
        })
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.notify
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub fn with_discover<Output>(
    window: &AppWindow,
    action: impl FnOnce(&DiscoverModel) -> Output,
) -> Option<Output> {
    let shelves = window.global::<DiscoverState>().get_shelves();
    shelves.as_any().downcast_ref::<DiscoverModel>().map(action)
}
