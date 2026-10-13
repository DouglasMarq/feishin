use std::collections::{HashSet, VecDeque};

use super::cards::CardKind;

pub const MAX_COVERS: usize = 48;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CoverKey {
    pub kind: CardKind,
    pub id: String,
}

impl CoverKey {
    pub fn new(kind: CardKind, id: impl Into<String>) -> Self {
        Self {
            kind,
            id: id.into(),
        }
    }
}

#[derive(Default)]
pub struct Covers {
    loaded: VecDeque<CoverKey>,
    pending: HashSet<CoverKey>,
}

impl Covers {
    pub fn wants(&mut self, key: CoverKey) -> bool {
        let Some(position) = self.loaded.iter().position(|loaded| *loaded == key) else {
            return self.pending.insert(key);
        };
        if let Some(recent) = self.loaded.remove(position) {
            self.loaded.push_back(recent);
        }
        false
    }

    pub fn loaded(&mut self, key: CoverKey) -> Option<CoverKey> {
        self.pending.remove(&key);
        self.loaded.push_back(key);
        (self.loaded.len() > MAX_COVERS)
            .then(|| self.loaded.pop_front())
            .flatten()
    }

    pub fn failed(&mut self, key: &CoverKey) {
        self.pending.remove(key);
    }

    pub fn forget(&mut self, kind: CardKind) {
        self.loaded.retain(|key| key.kind != kind);
        self.pending.retain(|key| key.kind != kind);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn album_key(number: usize) -> CoverKey {
        CoverKey::new(CardKind::Albums, format!("album-{number}"))
    }

    #[test]
    fn covers_are_bounded_and_deduplicated() {
        let mut covers = Covers::default();
        assert!(covers.wants(album_key(0)));
        assert!(!covers.wants(album_key(0)), "in flight");
        for number in 0..MAX_COVERS {
            assert_eq!(covers.loaded(album_key(number)), None);
        }
        assert!(
            !covers.wants(album_key(0)),
            "already loaded, now most recent"
        );
        assert_eq!(covers.loaded(album_key(MAX_COVERS)), Some(album_key(1)));
        covers.forget(CardKind::Albums);
        assert!(covers.wants(album_key(0)));
    }
}
