//! Play queue with Spotify-like semantics.
//!
//! Ported from `src/renderer/store/player.store.ts` plus the `feat/spotify-like-playback` behaviour:
//! - The queue keeps the original order. When shuffle is on, a permutation defines the play order,
//!   and that play order is also the display order (`display_order`).
//! - `play_now` replaces the queue. With shuffle on it reshuffles every time and puts the start song first.
//! - Turning shuffle off restores the original order; turning it on reshuffles with the current item first.
//! - Moving items in the shuffled view only reorders the permutation; the original order is untouched.

use std::collections::HashSet;
use std::hash::{BuildHasher, RandomState};

use serde::{Deserialize, Serialize};

use crate::model::Song;

#[cfg(test)]
mod tests;

/// Stable id of an entry in the queue. The same song can appear more than once with different uids.
pub type Uid = u64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueueItem {
    pub uid: Uid,
    pub song: Song,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RepeatMode {
    #[default]
    None,
    All,
    One,
}

/// Why the player is moving to another item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Advance {
    /// The current track finished on its own. `RepeatMode::One` repeats the same item.
    Auto,
    /// The user pressed next. `RepeatMode::One` still moves on.
    Manual,
}

/// Serializable state used to restore the queue on startup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueueSnapshot {
    pub items: Vec<QueueItem>,
    /// Play order as indexes into `items`; present only while shuffle is on.
    pub shuffled: Option<Vec<usize>>,
    pub current_uid: Option<Uid>,
    pub repeat: RepeatMode,
    pub next_uid: Uid,
}

#[derive(Debug, Clone)]
struct SplitMix64(u64);

impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        ((u128::from(self.next_u64()) * bound as u128) >> 64) as usize
    }

    fn shuffle<T>(&mut self, slice: &mut [T]) {
        for i in (1..slice.len()).rev() {
            let j = self.below(i + 1);
            slice.swap(i, j);
        }
    }
}

enum Step {
    To(usize),
    Stop,
}

#[derive(Debug, Clone)]
pub struct Queue {
    items: Vec<QueueItem>,
    order: Option<Vec<usize>>,
    current: Option<Uid>,
    repeat: RepeatMode,
    next_uid: Uid,
    rng: SplitMix64,
}

impl Default for Queue {
    fn default() -> Self {
        Self::new()
    }
}

impl Queue {
    pub fn new() -> Self {
        Self::with_seed(random_seed())
    }

    /// Same as `new` but with a deterministic shuffle, for tests.
    pub fn with_seed(seed: u64) -> Self {
        Self {
            items: Vec::new(),
            order: None,
            current: None,
            repeat: RepeatMode::None,
            next_uid: 1,
            rng: SplitMix64(seed),
        }
    }

    /// Items in their original (unshuffled) order.
    pub fn items(&self) -> &[QueueItem] {
        &self.items
    }

    /// Items in play order: the shuffled order while shuffle is on, the original order otherwise.
    pub fn display_order(&self) -> Vec<&QueueItem> {
        match &self.order {
            Some(order) => order.iter().map(|&i| &self.items[i]).collect(),
            None => self.items.iter().collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn current(&self) -> Option<&QueueItem> {
        self.current_index().map(|i| &self.items[i])
    }

    /// Position of the current item in `display_order`.
    pub fn current_position(&self) -> Option<usize> {
        self.current_index().and_then(|i| self.position_of_index(i))
    }

    pub fn shuffle(&self) -> bool {
        self.order.is_some()
    }

    pub fn set_shuffle(&mut self, on: bool) {
        if on == self.shuffle() {
            return;
        }
        self.order = on.then(|| self.shuffled_order(self.current_index()));
    }

    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }

    pub fn set_repeat(&mut self, mode: RepeatMode) {
        self.repeat = mode;
    }

    /// Replace the queue with `songs` and make `start_song_id` (or the first song) current.
    /// With shuffle on, the queue is reshuffled with the start song first.
    pub fn play_now(
        &mut self,
        songs: Vec<Song>,
        start_song_id: Option<&str>,
    ) -> Option<&QueueItem> {
        self.items = self.make_items(songs);
        let start = start_song_id.and_then(|id| self.items.iter().position(|it| it.song.id == id));
        if self.order.is_some() {
            let order = self.shuffled_order(start);
            self.current = order.first().map(|&i| self.items[i].uid);
            self.order = Some(order);
        } else {
            self.current = self.items.get(start.unwrap_or(0)).map(|it| it.uid);
        }
        self.current()
    }

    /// Insert right after the current item in play order.
    /// With shuffle on, the new items are shuffled among themselves, as in Feishin.
    pub fn add_next(&mut self, songs: Vec<Song>) {
        if songs.is_empty() {
            return;
        }
        let count = songs.len();
        let insert_at = self.current_index().map_or(0, |i| i + 1);
        let play_at = self.current_position().map_or(0, |p| p + 1);
        let new_items = self.make_items(songs);
        self.items.splice(insert_at..insert_at, new_items);

        if let Some(mut order) = self.order.take() {
            for idx in &mut order {
                if *idx >= insert_at {
                    *idx += count;
                }
            }
            let mut new_indexes: Vec<usize> = (insert_at..insert_at + count).collect();
            self.rng.shuffle(&mut new_indexes);
            order.splice(play_at..play_at, new_indexes);
            self.order = Some(order);
        }
    }

    /// Append at the end of play order.
    /// With shuffle on, the new items are shuffled among themselves, as in Feishin.
    pub fn add_last(&mut self, songs: Vec<Song>) {
        let start = self.items.len();
        let new_items = self.make_items(songs);
        self.items.extend(new_items);

        if let Some(mut order) = self.order.take() {
            let mut new_indexes: Vec<usize> = (start..self.items.len()).collect();
            self.rng.shuffle(&mut new_indexes);
            order.extend(new_indexes);
            self.order = Some(order);
        }
    }

    /// Move to the next item in play order, honouring repeat. Returns the new current item.
    ///
    /// At the end of the queue with `RepeatMode::None` playback should stop: the first item in play
    /// order becomes current and `None` is returned. With `RepeatMode::One` and `Advance::Manual`
    /// the last item stays current. With nothing current, the first item in play order is chosen.
    pub fn next(&mut self, advance: Advance) -> Option<&QueueItem> {
        match self.next_step(advance)? {
            Step::To(pos) => {
                self.current = Some(self.item_at(pos).uid);
                self.current()
            }
            Step::Stop => {
                self.current = Some(self.item_at(0).uid);
                None
            }
        }
    }

    /// Move to the previous item in play order (wraps with `RepeatMode::All`).
    ///
    /// On the first item without `RepeatMode::All` the first item stays current.
    pub fn previous(&mut self) -> Option<&QueueItem> {
        let len = self.items.len();
        if len == 0 {
            return None;
        }
        let pos = match self.current_position() {
            None => 0,
            Some(0) if self.repeat == RepeatMode::All => len - 1,
            Some(pos) => pos.saturating_sub(1),
        };
        self.current = Some(self.item_at(pos).uid);
        self.current()
    }

    /// The item that `next(Advance::Auto)` would return, for gapless preloading. Does not mutate.
    pub fn peek_next(&self) -> Option<&QueueItem> {
        match self.next_step(Advance::Auto)? {
            Step::To(pos) => Some(self.item_at(pos)),
            Step::Stop => None,
        }
    }

    pub fn play_uid(&mut self, uid: Uid) -> Option<&QueueItem> {
        self.index_of(uid)?;
        self.current = Some(uid);
        self.current()
    }

    /// Remove the given items. When the current item is removed, the following item in play order
    /// becomes current, or the first one when nothing follows.
    pub fn remove(&mut self, uids: &[Uid]) {
        let removed: HashSet<Uid> = uids.iter().copied().collect();
        if !self.items.iter().any(|it| removed.contains(&it.uid)) {
            return;
        }

        if self.current.is_some_and(|uid| removed.contains(&uid)) {
            let play: Vec<usize> = self.play_indexes();
            let from = self.current_position().map_or(0, |p| p + 1);
            let kept = |&&i: &&usize| !removed.contains(&self.items[i].uid);
            self.current = play[from..]
                .iter()
                .find(kept)
                .or_else(|| play.iter().find(kept))
                .map(|&i| self.items[i].uid);
        }

        let mut new_index = Vec::with_capacity(self.items.len());
        let mut next = 0;
        for item in &self.items {
            if removed.contains(&item.uid) {
                new_index.push(None);
            } else {
                new_index.push(Some(next));
                next += 1;
            }
        }
        if let Some(order) = &mut self.order {
            *order = order.iter().filter_map(|&i| new_index[i]).collect();
        }
        self.items.retain(|it| !removed.contains(&it.uid));
    }

    /// Move `uids` (keeping their relative order) before `before_uid` in play order, or to the end when `None`.
    ///
    /// The moved items keep the order they had in play order. Unknown uids are ignored, and an
    /// unknown `before_uid` leaves the queue untouched.
    pub fn move_items(&mut self, uids: &[Uid], before_uid: Option<Uid>) {
        let moving: HashSet<Uid> = uids.iter().copied().collect();
        let play = self.play_indexes();
        let is_moving = |i: usize| moving.contains(&self.items[i].uid);

        let anchor = match before_uid {
            None => None,
            Some(uid) => {
                let Some(target) = self.index_of(uid) else {
                    return;
                };
                let Some(pos) = play.iter().position(|&i| i == target) else {
                    return;
                };
                play[pos..].iter().copied().find(|&i| !is_moving(i))
            }
        };

        let (moved, mut rest): (Vec<usize>, Vec<usize>) = play.iter().partition(|&&i| is_moving(i));
        if moved.is_empty() {
            return;
        }
        let at = anchor
            .and_then(|a| rest.iter().position(|&i| i == a))
            .unwrap_or(rest.len());
        rest.splice(at..at, moved);

        if self.order.is_some() {
            self.order = Some(rest);
        } else {
            let mut old: Vec<Option<QueueItem>> = std::mem::take(&mut self.items)
                .into_iter()
                .map(Some)
                .collect();
            self.items = rest.iter().filter_map(|&i| old[i].take()).collect();
        }
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.current = None;
        if let Some(order) = &mut self.order {
            order.clear();
        }
    }

    pub fn snapshot(&self) -> QueueSnapshot {
        QueueSnapshot {
            items: self.items.clone(),
            shuffled: self.order.clone(),
            current_uid: self.current,
            repeat: self.repeat,
            next_uid: self.next_uid,
        }
    }

    /// Restore a queue. Inconsistent snapshots are repaired: duplicate uids get fresh ones, an
    /// unknown current uid is dropped and an invalid shuffle order is regenerated.
    pub fn from_snapshot(snapshot: QueueSnapshot) -> Self {
        let mut queue = Self::new();
        let QueueSnapshot {
            mut items,
            shuffled,
            current_uid,
            repeat,
            next_uid,
        } = snapshot;

        let max_uid = items.iter().map(|it| it.uid).max();
        queue.next_uid = next_uid
            .max(max_uid.map_or(0, |m| m.saturating_add(1)))
            .max(1);
        let mut seen = HashSet::with_capacity(items.len());
        for item in &mut items {
            if !seen.insert(item.uid) {
                item.uid = queue.alloc_uid();
            }
        }

        queue.items = items;
        queue.repeat = repeat;
        queue.current = current_uid.filter(|&uid| queue.index_of(uid).is_some());
        queue.order = shuffled.map(|order| {
            if is_permutation(&order, queue.items.len()) {
                order
            } else {
                queue.shuffled_order(queue.current_index())
            }
        });
        queue
    }

    fn alloc_uid(&mut self) -> Uid {
        let uid = self.next_uid;
        self.next_uid = self.next_uid.saturating_add(1);
        uid
    }

    fn make_items(&mut self, songs: Vec<Song>) -> Vec<QueueItem> {
        songs
            .into_iter()
            .map(|song| QueueItem {
                uid: self.alloc_uid(),
                song,
            })
            .collect()
    }

    fn index_of(&self, uid: Uid) -> Option<usize> {
        self.items.iter().position(|it| it.uid == uid)
    }

    fn current_index(&self) -> Option<usize> {
        self.current.and_then(|uid| self.index_of(uid))
    }

    fn position_of_index(&self, index: usize) -> Option<usize> {
        match &self.order {
            Some(order) => order.iter().position(|&i| i == index),
            None => (index < self.items.len()).then_some(index),
        }
    }

    fn item_at(&self, pos: usize) -> &QueueItem {
        match &self.order {
            Some(order) => &self.items[order[pos]],
            None => &self.items[pos],
        }
    }

    fn play_indexes(&self) -> Vec<usize> {
        match &self.order {
            Some(order) => order.clone(),
            None => (0..self.items.len()).collect(),
        }
    }

    /// A fresh random play order, with `first` (an index into `items`) at the front when given.
    fn shuffled_order(&mut self, first: Option<usize>) -> Vec<usize> {
        let len = self.items.len();
        match first.filter(|&f| f < len) {
            Some(first) => {
                let mut order: Vec<usize> = std::iter::once(first)
                    .chain((0..len).filter(|&i| i != first))
                    .collect();
                self.rng.shuffle(&mut order[1..]);
                order
            }
            None => {
                let mut order: Vec<usize> = (0..len).collect();
                self.rng.shuffle(&mut order);
                order
            }
        }
    }

    fn next_step(&self, advance: Advance) -> Option<Step> {
        let len = self.items.len();
        if len == 0 {
            return None;
        }
        let Some(pos) = self.current_position() else {
            return Some(Step::To(0));
        };
        Some(match (self.repeat, advance) {
            (RepeatMode::One, Advance::Auto) => Step::To(pos),
            (RepeatMode::One, Advance::Manual) => Step::To((pos + 1).min(len - 1)),
            (RepeatMode::All, _) => Step::To((pos + 1) % len),
            (RepeatMode::None, _) if pos + 1 < len => Step::To(pos + 1),
            (RepeatMode::None, _) => Step::Stop,
        })
    }
}

fn is_permutation(order: &[usize], len: usize) -> bool {
    if order.len() != len {
        return false;
    }
    let mut seen = vec![false; len];
    order
        .iter()
        .all(|&i| i < len && !std::mem::replace(&mut seen[i], true))
}

fn random_seed() -> u64 {
    RandomState::new().hash_one(0x5EED_u64)
}
