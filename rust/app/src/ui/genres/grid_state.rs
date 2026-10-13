use std::collections::{HashMap, HashSet};
use std::ops::Range;

use slint::Image;

use super::card::{CardData, CardSource};
use super::cover_cache::cached_cover;
use crate::ui::Card;

pub const GRID_PAGE: usize = 60;

pub enum GridRequest {
    Page {
        generation: u64,
        source: CardSource,
        page: usize,
    },
    Cover {
        cover_id: String,
    },
}

pub struct RowChange {
    pub old_rows: usize,
    pub new_rows: usize,
    pub changed: Range<usize>,
}

#[derive(Default)]
pub struct GridState {
    pub generation: u64,
    pub source: Option<CardSource>,
    pub columns: usize,
    total: usize,
    pages: HashMap<usize, Vec<CardData>>,
    requested: HashSet<usize>,
    covers_requested: HashSet<String>,
    overrides: HashMap<String, bool>,
}

impl GridState {
    pub fn columns(&self) -> usize {
        self.columns.max(1)
    }

    pub fn rows(&self) -> usize {
        self.total.div_ceil(self.columns())
    }

    fn cards(&self) -> impl Iterator<Item = (usize, &CardData)> {
        self.pages.iter().flat_map(|(page, cards)| {
            cards
                .iter()
                .enumerate()
                .map(move |(offset, card)| (page * GRID_PAGE + offset, card))
        })
    }

    fn card_with_id(&self, id: &str) -> Option<(usize, &CardData)> {
        self.cards().find(|(_, card)| card.id == id)
    }

    pub fn restarted(&self, source: Option<CardSource>) -> Self {
        Self {
            generation: self.generation + 1,
            columns: self.columns,
            source,
            ..Self::default()
        }
    }

    pub fn request_first_page(&mut self) -> Option<GridRequest> {
        let source = self.source.clone()?;
        self.requested.insert(0);
        Some(GridRequest::Page {
            generation: self.generation,
            source,
            page: 0,
        })
    }

    pub fn store_page(
        &mut self,
        page: usize,
        cards: Vec<CardData>,
        total: Option<usize>,
    ) -> RowChange {
        let start = page * GRID_PAGE;
        let end = start + cards.len();
        let old_rows = self.rows();
        self.total = self.total_after_page(total, cards.len(), end);
        self.pages.insert(page, cards);
        let columns = self.columns();
        RowChange {
            old_rows,
            new_rows: self.rows(),
            changed: start / columns..end.div_ceil(columns),
        }
    }

    fn total_after_page(&self, total: Option<usize>, card_count: usize, end: usize) -> usize {
        match total {
            Some(total) => total,
            None if card_count == GRID_PAGE => self.total.max(end + 1),
            None => end,
        }
    }

    pub fn forget_page_request(&mut self, generation: u64, page: usize) -> bool {
        self.generation == generation && self.requested.remove(&page)
    }

    pub fn cover_loaded(&mut self, cover_id: &str) -> HashSet<usize> {
        self.covers_requested.remove(cover_id);
        let columns = self.columns();
        self.cards()
            .filter(|(_, card)| card.cover_id.as_deref() == Some(cover_id))
            .map(|(index, _)| index / columns)
            .collect()
    }

    pub fn favorite(&self, id: &str) -> Option<bool> {
        let (_, card) = self.card_with_id(id)?;
        Some(self.overrides.get(id).copied().unwrap_or(card.favorite))
    }

    pub fn override_favorite(&mut self, id: &str, favorite: bool) -> Option<usize> {
        self.overrides.insert(id.to_owned(), favorite);
        let columns = self.columns();
        self.card_with_id(id).map(|(index, _)| index / columns)
    }

    pub fn cards_for_row(
        &mut self,
        row: usize,
        requests: &mut Vec<GridRequest>,
    ) -> Option<Vec<Card>> {
        let columns = self.columns();
        let start = row * columns;
        if start >= self.total {
            return None;
        }
        let end = (start + columns).min(self.total);
        Some(
            (start..end)
                .map(|index| self.card_at(index, requests))
                .collect(),
        )
    }

    fn card_at(&mut self, index: usize, requests: &mut Vec<GridRequest>) -> Card {
        let page = index / GRID_PAGE;
        let loaded = self
            .pages
            .get(&page)
            .and_then(|cards| cards.get(index % GRID_PAGE));
        let Some(data) = loaded else {
            requests.extend(self.request_page(page));
            return Card::default();
        };
        let cover = data.cover_id.as_deref().and_then(cached_cover);
        if cover.is_none()
            && let Some(cover_id) = &data.cover_id
            && self.covers_requested.insert(cover_id.clone())
        {
            requests.push(GridRequest::Cover {
                cover_id: cover_id.clone(),
            });
        }
        let favorite = self
            .overrides
            .get(&data.id)
            .copied()
            .unwrap_or(data.favorite);
        loaded_card(data, cover, favorite)
    }

    fn request_page(&mut self, page: usize) -> Option<GridRequest> {
        let source = self.source.as_ref()?;
        self.requested.insert(page).then(|| GridRequest::Page {
            generation: self.generation,
            source: source.clone(),
            page,
        })
    }
}

fn loaded_card(data: &CardData, cover: Option<Image>, favorite: bool) -> Card {
    Card {
        id: data.id.as_str().into(),
        title: data.title.as_str().into(),
        subtitle: data.subtitle.as_str().into(),
        has_cover: cover.is_some(),
        cover: cover.unwrap_or_default(),
        favorite,
        loaded: true,
    }
}
