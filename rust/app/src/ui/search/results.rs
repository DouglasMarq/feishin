use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, Image, Model, ModelTracker, VecModel};

use super::cards::CardKind;
use super::covers::{CoverKey, Covers};
use super::searches::Searches;
use super::section::Applied;
use crate::ui::{AppWindow, SearchCard, SearchKind, SearchState, TrackRow};

#[derive(Default)]
pub struct Results {
    pub searches: RefCell<Searches>,
    pub covers: RefCell<Covers>,
    songs: VecModel<TrackRow>,
    pub albums: Rc<VecModel<SearchCard>>,
    pub artists: Rc<VecModel<SearchCard>>,
}

impl Model for Results {
    type Data = TrackRow;

    fn row_count(&self) -> usize {
        self.songs.row_count()
    }

    fn row_data(&self, row: usize) -> Option<TrackRow> {
        self.songs.row_data(row)
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        self.songs.model_tracker()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Results {
    fn cards(&self, kind: CardKind) -> &VecModel<SearchCard> {
        match kind {
            CardKind::Albums => &self.albums,
            CardKind::Artists => &self.artists,
        }
    }

    pub fn begin_search(
        &self,
        window: &AppWindow,
        query: Option<String>,
        force: bool,
    ) -> Option<u64> {
        let cleared = query.is_none();
        let generation = self.searches.borrow_mut().begin(query, force)?;
        if cleared {
            self.show_sections();
        }
        self.show_flags(window);
        Some(generation)
    }

    pub fn show_all(&self, window: &AppWindow) {
        self.show_sections();
        self.show_flags(window);
    }

    fn show_sections(&self) {
        for kind in [SearchKind::Songs, SearchKind::Albums, SearchKind::Artists] {
            self.show_section(kind, 0);
        }
    }

    pub fn show_more(&self, window: &AppWindow, kind: SearchKind, applied: &Applied) {
        self.show_section(kind, applied.first_new_index());
        self.show_flags(window);
    }

    fn show_section(&self, kind: SearchKind, start: usize) {
        match kind {
            SearchKind::Songs => self.show_songs(start),
            SearchKind::Albums => self.show_cards(CardKind::Albums, start),
            SearchKind::Artists => self.show_cards(CardKind::Artists, start),
        }
    }

    fn show_songs(&self, start: usize) {
        let rows = self.searches.borrow().song_rows_from(start);
        match start {
            0 => self.songs.set_vec(rows),
            _ => self.songs.extend(rows),
        }
    }

    fn show_cards(&self, kind: CardKind, start: usize) {
        let cards = self.searches.borrow().cards_from(kind, start);
        match start {
            0 => {
                self.covers.borrow_mut().forget(kind);
                self.cards(kind).set_vec(cards);
            }
            _ => self.cards(kind).extend(cards),
        }
    }

    pub fn show_flags(&self, window: &AppWindow) {
        let searches = self.searches.borrow();
        let state = window.global::<SearchState>();
        state.set_loading(searches.pending);
        state.set_songs_more(searches.songs.has_more);
        state.set_albums_more(searches.albums.has_more);
        state.set_artists_more(searches.artists.has_more);
        state.set_songs_busy(searches.songs.busy);
        state.set_albums_busy(searches.albums.busy);
        state.set_artists_busy(searches.artists.busy);
    }

    pub fn claim_cover(&self, kind: CardKind, index: usize) -> Option<(String, String)> {
        let (id, cover_id) = self.searches.borrow().cover_source(kind, index)?;
        let wanted = self
            .covers
            .borrow_mut()
            .wants(CoverKey::new(kind, id.clone()));
        wanted.then_some((id, cover_id))
    }

    pub fn set_cover(&self, kind: CardKind, index: usize, id: &str, cover: Option<Image>) {
        let key = CoverKey::new(kind, id);
        let cards = self.cards(kind);
        let target = cards.row_data(index).filter(|card| card.id == id);
        let (Some(mut card), Some(cover)) = (target, cover) else {
            self.covers.borrow_mut().failed(&key);
            return;
        };
        card.cover = cover;
        card.has_cover = true;
        cards.set_row_data(index, card);
        let evicted = self.covers.borrow_mut().loaded(key);
        if let Some(evicted) = evicted {
            self.drop_cover(&evicted);
        }
    }

    fn drop_cover(&self, key: &CoverKey) {
        let cards = self.cards(key.kind);
        let Some(index) = cards.iter().position(|card| card.id == key.id.as_str()) else {
            return;
        };
        cards.set_row_data(
            index,
            SearchCard {
                cover: Image::default(),
                has_cover: false,
                ..cards.row_data(index).unwrap_or_default()
            },
        );
    }
}

pub fn with_results<Output>(
    window: &AppWindow,
    action: impl FnOnce(&Results) -> Output,
) -> Option<Output> {
    let songs = window.global::<SearchState>().get_songs();
    songs.as_any().downcast_ref::<Results>().map(action)
}
