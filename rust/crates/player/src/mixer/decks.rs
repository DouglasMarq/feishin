use crate::crossfade::DeckSlot;
use crate::deck::Deck;

#[derive(Default)]
pub(crate) struct Decks {
    first: Option<Deck>,
    second: Option<Deck>,
}

impl Decks {
    pub(crate) fn get(&self, slot: DeckSlot) -> Option<&Deck> {
        match slot {
            DeckSlot::First => self.first.as_ref(),
            DeckSlot::Second => self.second.as_ref(),
        }
    }

    pub(crate) fn contains(&self, slot: DeckSlot) -> bool {
        self.get(slot).is_some()
    }

    pub(crate) fn insert(&mut self, slot: DeckSlot, deck: Deck) {
        match slot {
            DeckSlot::First => self.first = Some(deck),
            DeckSlot::Second => self.second = Some(deck),
        }
    }
}
